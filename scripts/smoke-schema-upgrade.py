"""Rehearse a pre-Pages native package upgrade using disposable copies only."""

import argparse
from hashlib import file_digest
import importlib.util
import json
from pathlib import Path
import re
import shutil
import socket
from tempfile import TemporaryDirectory


spec = importlib.util.spec_from_file_location(
    "legura_http_smoke", Path(__file__).with_name("smoke-clean-linux.py")
)
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)


def settings(path):
    return dict(line.split("=", 1) for line in path.read_text().splitlines())


def hashes(paths):
    def digest(path):
        with path.open("rb") as handle:
            return file_digest(handle, "sha256").hexdigest()

    return [digest(path) for path in paths]


def seed_prior(package, port):
    data = package / "data"
    media = data / "media"
    media.mkdir(parents=True, mode=0o700)
    data.chmod(0o700)
    database = data / "legura.db"
    fixture.maintenance(package, "init", database)
    fixture.maintenance(package, "config-local", package)
    config = package / ".env"
    token = settings(config)["AX_SECRET_SETUP_TOKEN"]
    with config.open("a") as out:
        out.write("AX_SECRET_MEDIA_DRIVER=local\n"
                  f"AX_SECRET_MEDIA_ROOT={media.as_posix()}\n"
                  "AX_SECRET_MEDIA_STORE_ID=local-media\n")
    with fixture.server(package, port):
        client = fixture.Client()
        client.action("/setup", "Install", fields={
            "siteName": "Schema upgrade proof", "email": fixture.EMAIL,
            "password": fixture.PASSWORD, "setupToken": token,
        })
        client.action("/admin/posts/new", "CreatePost", fields={
            "title": "Clean Linux proof", "slug": "clean-linux",
            "body": "Native package proof", "status": "published",
        })
        listing, _ = client.request("/admin/posts", 200)
        post = re.search(rb'/admin/posts/([0-9a-f-]{36})/edit', listing)
        assert post, "Prior package did not list its published post"
        post_id = post[1].decode()
        result, _ = client.action("/admin/media", "UploadLibraryImage", 200, upload=fixture.PNG)
        uploaded = json.loads(result)
        assert uploaded["ok"] and uploaded["value"]["storage"] == "local-media"
        image_id = uploaded["value"]["id"]
        client.action(f"/admin/posts/{post_id}/image", "SetPostImage", fields={
            "id": post_id, "media_id": image_id, "alt": "Preserve this image",
        })
        image_url = f"/api/post-images/clean-linux/{image_id}"
        assert fixture.verify_public(image_url) == (media / image_id).read_bytes()
    return database, media, image_id, image_url


def login():
    client = fixture.Client()
    client.request("/admin", 403)
    client.action("/login", "SignIn", fields={"email": fixture.EMAIL, "password": fixture.PASSWORD})
    client.request("/admin", 200)
    return client


def rehearse(prior_package, candidate_package, root, port):
    if prior_package == candidate_package:
        raise ValueError("Use separate prior and candidate packages")
    prior = root / "prior"
    candidate = root / "candidate"
    for package, copy in ((prior_package, prior), (candidate_package, candidate)):
        if (package / ".env").exists() or (package / "data").exists():
            raise ValueError("Use fresh, unconfigured packages")
        fixture.maintenance(package, "verify-package", package)
        shutil.copytree(package, copy)

    database, media, image_id, image_url = seed_prior(prior, port)
    archive = root / "before.legura"
    fixture.maintenance(prior, "archive-backup", database, "local-media", media, archive)
    original = [database, media / image_id, archive, prior / ".env"]
    before = hashes(original)

    workspace = root / "rehearsal"
    report = json.loads(fixture.maintenance(candidate, "upgrade-rehearse", archive, workspace, capture=True))
    assert report["migrations_applied"] == 4 and report["archive"]["files"] == 2, report
    migrated = workspace / "rehearsed.legura"
    recovery = workspace / "verified-recovery"
    fixture.maintenance(candidate, "upgrade-preflight", candidate, migrated, recovery)
    assert not (candidate / ".env").exists(), "Preflight configured the candidate"
    fixture.maintenance(candidate, "config-recovery", migrated, recovery, candidate, "local-http")
    assert settings(candidate / ".env")["AX_SECRET_SESSION_KEY"] != settings(prior / ".env")["AX_SECRET_SESSION_KEY"]
    restored_image = recovery / "media" / "local-media" / image_id
    assert hashes([restored_image])[0] == before[1], "Migrated archive changed image bytes"

    with fixture.server(candidate, port):
        admin = login()
        admin.request("/setup", 403)
        assert fixture.verify_public(image_url) == restored_image.read_bytes()
        fixture.verify_pages(admin)
        fixture.verify_navigation(admin)
        fixture.verify_public_interface(admin)
        fixture.verify_languages(admin)
        admin.action("/admin/language", "SetAdminLanguage", fields={"admin_language": "sr-Latn"})
    with fixture.server(candidate, port):
        admin = login()
        page, _ = admin.request("/admin/language", 200)
        assert b"Primeni jezik panela" in page
        fixture.verify_site_language(fixture.Client())
        fixture.verify_navigation_persistence(fixture.Client())
        assert fixture.verify_public(image_url) == restored_image.read_bytes()
    assert hashes(original) == before, "Rehearsal changed the stopped source or backup"


if __name__ == "__main__":
    if not __debug__:
        raise RuntimeError("Run this smoke with Python assertions enabled")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("prior", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--port", type=int, default=3968)
    args = parser.parse_args()
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", args.port))
    fixture.BASE = f"http://127.0.0.1:{args.port}"
    with TemporaryDirectory(prefix="legura-schema-upgrade-") as scratch:
        rehearse(args.prior.resolve(strict=True), args.candidate.resolve(strict=True), Path(scratch), args.port)
    print("Schema upgrade passed: four migrations, preserved source, content/media, and restarted menus/languages.")
