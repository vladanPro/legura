"""Exercise a trusted CI-built native archive inside a clean Linux container."""

import base64
from contextlib import contextmanager
from hashlib import file_digest
from http.cookiejar import CookieJar
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
from tempfile import TemporaryDirectory
import time
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import HTTPCookieProcessor, HTTPRedirectHandler, Request, build_opener


BASE = "http://127.0.0.1:3940"
EMAIL = "native@example.test"
PASSWORD = "NativePilot9!"
PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4"
    "nGNwqzjxHwAFFAKG7uYx/AAAAABJRU5ErkJggg=="
)


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


class Client:
    def __init__(self):
        self.opener = build_opener(NoRedirect(), HTTPCookieProcessor(CookieJar()))

    def request(self, path, expected, body=None, headers=None):
        request = Request(BASE + path, data=body, headers=headers or {})
        try:
            response = self.opener.open(request, timeout=3)
        except HTTPError as error:
            response = error
        with response:
            assert response.code == expected, f"{path}: expected {expected}, got {response.code}"
            return response.read(), response.headers

    def action(self, route, name, expected=303, fields=None, upload=None):
        csrf, _ = self.request("/__axonyx/csrf", 200, headers={"Origin": BASE})
        headers = {
            "Origin": BASE,
            "X-Axonyx-CSRF": json.loads(csrf)["token"],
            "Accept": "text/html",
            "Content-Type": "application/x-www-form-urlencoded",
        }
        body = urlencode(fields or {}).encode()
        if upload is not None:
            headers["Accept"] = "application/ax-patch+json"
            headers["Content-Type"] = "multipart/form-data; boundary=legura-smoke"
            body = (
                b'--legura-smoke\r\nContent-Disposition: form-data; name="image"; filename="fixture.png"\r\n'
                b"Content-Type: image/png\r\n\r\n" + upload + b"\r\n--legura-smoke--\r\n"
            )
        query = urlencode({"path": route, "name": name})
        return self.request(f"/__axonyx/action?{query}", expected, body, headers)


def unpack(archive, package):
    package.mkdir()
    with tarfile.open(archive, "r:gz") as tar:
        tar.extractall(package, filter="data")
    assert not (package / ".env").exists()
    assert not (package / "data").exists()


def maintenance(package, *args):
    subprocess.run([package / "legura-maintenance", *args], cwd=package, check=True, timeout=60)


@contextmanager
def server(package):
    env = {"PATH": os.environ["PATH"], "AXONYX_HOST": "127.0.0.1", "AXONYX_PORT": "3940"}
    with subprocess.Popen([package / "legura-server"], cwd=package, env=env) as process:
        try:
            for _ in range(120):
                assert process.poll() is None, "Native server exited before readiness"
                try:
                    Client().request("/__axonyx/ready", 200)
                    break
                except URLError:
                    time.sleep(0.25)
            else:
                raise RuntimeError("Native server did not become ready")
            yield
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    raise


def verify_public(image_url):
    guest = Client()
    page, _ = guest.request("/posts/clean-linux", 200)
    assert b"Native package proof" in page and image_url.encode() in page
    image, headers = guest.request(image_url, 200)
    assert headers.get_content_type() == "image/png"
    return image


def verify_pages(client):
    guest = Client()
    guest.request("/admin/pages", 403)
    guest.request("/admin/pages/new", 403)
    client.request("/admin/pages", 200)
    client.request("/admin/pages/new", 200)
    draft = {"title": "About us", "slug": "o-nama", "body": "Page proof <script>fixture()</script>"}
    guest.action("/admin/pages/new", "CreatePage", 403, fields=draft)
    client.action("/admin/pages/new", "CreatePage", 422, fields=dict(draft, slug="login"))
    client.action("/admin/pages/new", "CreatePage", 422, fields=dict(draft, slug="a/b"))
    _, headers = client.action("/admin/pages/new", "CreatePage", fields=draft)
    assert headers["Location"] == "/admin/pages"
    listing, _ = client.request("/admin/pages", 200)
    match = re.search(rb'/admin/pages/[0-9a-f-]{36}/edit', listing)
    assert match and b"About us" in listing
    edit = match[0].decode()
    preview = edit.removesuffix("edit") + "preview"
    client.request(edit, 200)
    guest.request(edit, 403)
    guest.request(preview, 403)
    guest.request("/o-nama", 404)
    saved, _ = client.request(preview, 200)
    assert b"Private preview" in saved and b"Page proof" in saved
    assert b"<script>fixture()</script>" not in saved
    client.action("/admin/pages/new", "CreatePage", 422, fields=draft)
    fields = {
        "id": edit.split("/")[-2],
        "title": draft["title"], "body": draft["body"], "status": "published",
    }
    guest.action(edit, "UpdatePage", 403, fields=fields)
    client.action(edit, "UpdatePage", fields=fields)
    verify_public_page(guest)
    client.action(edit, "UpdatePage", fields=dict(fields, status="draft"))
    guest.request("/o-nama", 404)
    client.action(edit, "UpdatePage", fields=fields)


def verify_public_page(guest):
    page, _ = guest.request("/o-nama", 200)
    assert b"About us" in page and b"Page proof" in page
    assert b"<script>fixture()</script>" not in page
    for asset in ("legura.css", "legura-mark.svg", "appearance.js"):
        guest.request(f"/assets/{asset}", 200)
    guest.request("/login", 200)


def smoke(archive, root):
    original = root / "package"
    candidate = root / "recovered-package"
    unpack(archive, original)
    unpack(archive, candidate)
    maintenance(original, "verify-package", original)
    data = original / "data"
    data.mkdir(mode=0o700)
    media = data / "media"
    media.mkdir(mode=0o700)
    database = data / "legura.db"
    maintenance(original, "init", database)
    maintenance(original, "config-local", original)
    config = original / ".env"
    assert config.stat().st_mode & 0o077 == 0, "Configuration is not private"
    values = dict(line.split("=", 1) for line in config.read_text().splitlines())
    with config.open("a") as out:
        out.write(
            "AX_SECRET_MEDIA_DRIVER=local\n"
            f"AX_SECRET_MEDIA_ROOT={media}\n"
            "AX_SECRET_MEDIA_STORE_ID=local-media\n"
        )
    original_config = config.read_bytes()
    with server(original):
        client = Client()
        client.request("/setup", 200)
        client.action("/setup", "Install", fields={
            "siteName": "Clean Linux", "email": EMAIL, "password": PASSWORD,
            "setupToken": values["AX_SECRET_SETUP_TOKEN"],
        })
        client.request("/admin", 200)
        client.request("/setup", 403)
        verify_pages(client)
        client.action("/admin/posts/new", "CreatePost", fields={
            "title": "Clean Linux proof", "slug": "clean-linux",
            "body": "Native package proof", "status": "published",
        })
        listing, _ = client.request("/admin/posts", 200)
        match = re.search(rb'/admin/posts/([0-9a-f-]{36})/edit', listing)
        assert match, "Could not find the published post edit link"
        post_id = match[1].decode()
        result, _ = client.action("/admin/media", "UploadLibraryImage", 200, upload=PNG)
        upload = json.loads(result)
        assert upload["ok"] and upload["value"]["storage"] == "local-media"
        image_id = upload["value"]["id"]
        client.action(f"/admin/posts/{post_id}/image", "SetPostImage", fields={
            "id": post_id, "media_id": image_id, "alt": "Fixture image",
        })
        image_url = f"/api/post-images/clean-linux/{image_id}"
        image = verify_public(image_url)
        assert image == (media / image_id).read_bytes()

    backup = root / "backup.legura"
    recovery = root / "inactive"
    maintenance(original, "archive-backup", database, "local-media", media, backup)
    maintenance(candidate, "archive-verify", backup)
    maintenance(candidate, "archive-restore", backup, recovery)
    maintenance(candidate, "upgrade-preflight", candidate, backup, recovery)
    maintenance(candidate, "config-recovery", backup, recovery, candidate, "local-http")
    recovered_config = dict(line.split("=", 1) for line in (candidate / ".env").read_text().splitlines())
    assert recovered_config["AX_SECRET_SESSION_KEY"] != values["AX_SECRET_SESSION_KEY"]
    with server(candidate):
        assert verify_public(image_url) == image
        verify_public_page(Client())
        client.request("/admin", 403)  # The original session must not carry over.
        client.action("/login", "SignIn", fields={"email": EMAIL, "password": PASSWORD})
        client.request("/admin", 200)
        client.request("/setup", 403)
    assert config.read_bytes() == original_config
    print("Clean Linux package passed: setup, published post/image, backup, recovery and fresh login.")


if __name__ == "__main__":
    assert sys.platform == "linux" and os.geteuid() != 0
    for tool in ("rustc", "cargo", "cargo-ax", "node", "git", "pwsh"):
        assert shutil.which(tool) is None, f"Unexpected tool in runtime image: {tool}"
    archive = Path(sys.argv[1]).resolve(strict=True)
    checksum = archive.with_name(archive.name + ".sha256").read_text().split()
    with archive.open("rb") as source:
        assert checksum == [file_digest(source, "sha256").hexdigest(), archive.name]
    with TemporaryDirectory(prefix="legura-clean-linux-") as directory:
        smoke(archive, Path(directory))
