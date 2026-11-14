"""Rehearse a pre-Pages native package upgrade using disposable copies only."""

import argparse
from contextlib import closing
from datetime import datetime, timezone
from hashlib import file_digest
import importlib.util
import json
from pathlib import Path
import re
import shutil
import socket
import sqlite3
from tempfile import TemporaryDirectory


spec = importlib.util.spec_from_file_location(
    "legura_http_smoke", Path(__file__).with_name("smoke-native-archive.py")
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


def verify_trash_counts(admin, posts, pages):
    overview, _ = admin.request("/admin", 200)
    for kind, count in (("Posts", posts), ("Pages", pages)):
        link = f'href="/admin/{kind.lower()}?status=trash"'
        label = f"{kind} in Trash: {count}"
        assert re.search(rb'<a[^>]*' + re.escape(link.encode()) + rb'[^>]*>\s*' + label.encode() + rb'\s*</a>', overview), label


def verify_page_trash(admin, database):
    guest = fixture.Client()
    route = "/admin/pages"
    public = "/pages/o-nama"
    listing, _ = admin.request(route, 200)
    match = re.search(rb'href="(/admin/pages/[0-9a-f-]{36}/edit)"[^>]*>\s*About us\s*</a>', listing)
    assert match
    edit = match[1].decode()
    fields = {"id": edit.split("/")[-2]}
    delete = edit.removesuffix("edit") + "delete"
    update = dict(fields, title="About us", body="Page proof", status="published")

    admin.request(delete, 404)
    admin.action(delete, "DeletePage", 404, fields=dict(fields, confirmSlug="o-nama"))
    guest.action(route, "TrashPage", 403, fields=fields)
    editor, _ = admin.request(edit, 200)
    assert b"Move to Trash" in editor
    _, headers = admin.action(route, "TrashPage", fields=fields)
    assert headers["Location"] == route + "?status=trash"
    verify_trash_counts(admin, posts=0, pages=1)
    guest.request(public, 404)
    active, _ = admin.request(route, 200)
    assert edit.encode() not in active
    trash, _ = admin.request(route + "?status=trash", 200)
    assert delete.encode() in trash and b"Restore as draft" in trash
    admin.request(edit, 404)
    admin.request(edit.removesuffix("edit") + "preview", 404)
    admin.action(edit, "UpdatePage", 404, fields=update)
    guest.action(route, "RestorePage", 403, fields=fields)

    def assert_hidden():
        assert fixture.public_menu(guest, b"Public navigation") == [b"/pages/read-me"]
        assert not fixture.public_menu(guest, b"Footer navigation")

    assert_hidden()
    admin.action(route, "RestorePage", fields=fields)
    verify_trash_counts(admin, posts=0, pages=0)
    guest.request(public, 404)
    assert_hidden()
    admin.request(edit, 200)
    with closing(sqlite3.connect(database)) as db:
        assert db.execute("SELECT status, trashed FROM legura_pages WHERE id = ?", (fields["id"],)).fetchone() == ("draft", 0)
    admin.action(edit, "UpdatePage", fields=update)
    fixture.verify_navigation_persistence(guest)

    admin.action(route, "TrashPage", fields=fields)
    admin.request(delete, 200)
    admin.action(delete, "DeletePage", 422, fields=dict(fields, confirmSlug="wrong"))
    guest.action(delete, "DeletePage", 403, fields=dict(fields, confirmSlug="o-nama"))
    admin.action(delete, "DeletePage", fields=dict(fields, confirmSlug="o-nama"))
    verify_trash_counts(admin, posts=0, pages=0)
    admin.request(delete, 404)
    guest.request(public, 404)
    assert_hidden()
    with closing(sqlite3.connect(database)) as db:
        remaining, = db.execute("SELECT COUNT(*) FROM legura_menu_items WHERE page_id = ?", (fields["id"],)).fetchone()
        assert remaining == 0, "A permanently deleted page left menu references"


def verify_post_creation(admin, database):
    before = datetime.now(timezone.utc).replace(microsecond=0)
    fields = {"title": "Writer proof", "slug": "writer-proof", "body": "New post", "status": "published"}
    admin.action("/admin/posts/new", "CreatePost", fields=dict(fields, created_by="untrusted", created_at="2000-01-01"))
    with closing(sqlite3.connect(database)) as db:
        assert db.execute("SELECT created_by, created_at FROM legura_posts WHERE slug = 'clean-linux'").fetchone() == (None, None)
        post_id, author_id, author, created = db.execute(
            """SELECT p.id, p.created_by, u.email, p.created_at FROM legura_posts p
            JOIN legura_users u ON u.id = p.created_by WHERE p.slug = 'writer-proof'"""
        ).fetchone()
    assert author == fixture.EMAIL
    assert before <= datetime.fromisoformat(created) <= datetime.now(timezone.utc)
    listing, _ = admin.request("/admin/posts", 200)
    assert b"Unknown" in listing and author.encode() in listing
    assert f'datetime="{created}"'.encode() in listing
    assert created[:10].encode() in listing
    story, _ = fixture.Client().request("/posts/writer-proof", 200)
    assert author.encode() not in story
    admin.action(f"/admin/posts/{post_id}/edit", "UpdatePost", fields=dict(fields, id=post_id, title="Updated proof"))
    admin.action("/admin/posts", "TrashPost", fields={"id": post_id})
    admin.action("/admin/posts", "RestorePost", fields={"id": post_id})
    with closing(sqlite3.connect(database)) as db:
        assert db.execute("SELECT created_by, created_at FROM legura_posts WHERE id = ?", (post_id,)).fetchone() == (author_id, created)


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
    assert report["migrations_applied"] == 9 and report["archive"]["files"] == 2, report
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
        library, _ = admin.request("/admin/media", 200)
        assert f'data-media-id="{image_id}"'.encode() in library
        assert b'type="file"' in library
        fixture.Client().request("/admin/media", 403)
        fixture.verify_pages(admin)
        fixture.verify_navigation(admin)
        fixture.verify_public_interface(admin)
        fixture.verify_languages(admin)
        admin.action("/admin/language", "SetAdminLanguage", fields={"admin_language": "sr-Latn"})
        guest = fixture.Client()
        story, _ = guest.request("/posts/clean-linux", 200)
        assert b'data-foundry-mode="light"' in story
        assert b'data-foundry="silver"' in story
        admin.action("/admin/appearance", "SetPublicColorMode", fields={"color_mode": "dark"})
        admin.action("/admin/appearance", "SetPublicPalette", fields={"palette": "gold"})
    with fixture.server(candidate, port):
        admin = login()
        page, _ = admin.request("/admin/language", 200)
        assert b"Primeni jezik panela" in page
        fixture.verify_site_language(fixture.Client())
        fixture.verify_navigation_persistence(fixture.Client())
        assert fixture.verify_public(image_url) == restored_image.read_bytes()
        for route in ("/", "/posts", "/posts/clean-linux", "/pages/o-nama"):
            public, _ = fixture.Client().request(route, 200)
            assert re.search(rb'<div(?=[^>]*data-legura-public-theme="default")(?=[^>]*data-foundry="gold")[^>]*data-foundry-mode="dark"', public), route
        posts, _ = admin.request("/admin/posts", 200)
        post = re.search(rb'/admin/posts/([0-9a-f-]{36})/edit', posts)
        assert post
        post_id = post[1].decode()
        verify_trash_counts(admin, posts=0, pages=0)
        admin.action("/admin/posts", "TrashPost", fields={"id": post_id})
        verify_trash_counts(admin, posts=1, pages=0)
        guest = fixture.Client()
        guest.request("/posts/clean-linux", 404)
        guest.request(image_url, 404)
        admin.action("/admin/posts", "RestorePost", fields={"id": post_id})
        guest.request("/posts/clean-linux", 404)
        guest.request(image_url, 404)
        editor, _ = admin.request(f"/admin/posts/{post_id}/edit", 200)
        assert f'/api/media/{image_id}'.encode() in editor
        assert b"Preserve this image" in editor
        verify_page_trash(admin, recovery / "database.sqlite")
        verify_post_creation(admin, recovery / "database.sqlite")
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
    print("Schema upgrade passed: nine migrations, preserved source, content/media, page Trash, and post attribution.")
