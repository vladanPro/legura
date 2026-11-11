"""Exercise languages and public navigation on a fresh local source build."""

import argparse
from contextlib import contextmanager
import importlib.util
import os
from pathlib import Path
import subprocess
import time
from urllib.error import URLError


spec = importlib.util.spec_from_file_location(
    "legura_http_smoke", Path(__file__).with_name("smoke-clean-linux.py")
)
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)


@contextmanager
def server(binary, root, port):
    env = {
        key: value for key, value in os.environ.items()
        if not key.startswith(("AX_", "AXONYX_"))
    }
    env.update(AXONYX_HOST="127.0.0.1", AXONYX_PORT=str(port))
    with subprocess.Popen(
        [binary], cwd=root, env=env,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
    ) as process:
        try:
            for _ in range(100):
                assert process.poll() is None, "Compiled server exited before readiness"
                try:
                    fixture.Client().request("/__axonyx/ready", 200)
                    break
                except URLError:
                    time.sleep(0.2)
            else:
                raise RuntimeError("Compiled server did not become ready")
            yield
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=10)


def exercise_preferences(binary, root, port):
    setup_token = os.environ["LEGURA_PILOT_SETUP_TOKEN"]
    with server(binary, root, port):
        admin = fixture.Client()
        admin.request("/setup", 200)
        admin.action("/setup", "Install", fields={
            "siteName": "Language pilot", "email": fixture.EMAIL,
            "password": fixture.PASSWORD, "setupToken": setup_token,
        })
        fixture.verify_pages(admin)
        fixture.verify_navigation(admin)
        admin.action("/admin/posts/new", "CreatePost", fields={
            "title": "Clean Linux proof", "slug": "clean-linux",
            "body": "Language pilot proof", "status": "published",
        })
        fixture.verify_languages(admin)
        admin.action("/admin/language", "SetAdminLanguage", fields={"admin_language": "sr-Latn"})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--port", type=int, default=3965)
    parser.add_argument("--expect-fallback", action="store_true")
    parser.add_argument("--verify-existing", action="store_true", help="Reuse the seeded pilot database")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    binary = args.binary.resolve(strict=True)
    fixture.BASE = f"http://127.0.0.1:{args.port}"
    if not args.verify_existing:
        exercise_preferences(binary, root, args.port)
    with server(binary, root, args.port):
        admin = fixture.Client()
        admin.action("/login", "SignIn", fields={
            "email": fixture.EMAIL, "password": fixture.PASSWORD,
        })
        settings, _ = admin.request("/admin/language", 200)
        assert b"Primeni jezik panela" in settings, "Admin locale was lost on restart"
        hint = (
            b"Labels public post and page content for assistive technology."
            if args.expect_fallback else b"Oznaka jezika za javne objave i stranice."
        )
        assert hint in settings, "Catalog message or fallback was not rendered"
        fixture.verify_site_language(fixture.Client())
        fixture.verify_navigation_persistence(fixture.Client())
    print("Legura pilot passed: language catalogs, navigation, and restart persistence.")


if __name__ == "__main__":
    main()
