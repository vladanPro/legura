"""Regression checks for the native upgrade smoke's migration expectations."""

from contextlib import closing
import importlib.util
from pathlib import Path
import sqlite3
from tempfile import TemporaryDirectory
import unittest


spec = importlib.util.spec_from_file_location(
    "legura_schema_upgrade",
    Path(__file__).resolve().parents[1] / "scripts" / "smoke-schema-upgrade.py",
)
upgrade = importlib.util.module_from_spec(spec)
spec.loader.exec_module(upgrade)


class MigrationExpectations(unittest.TestCase):
    def test_count_tracks_candidate_history_without_a_fixed_total(self):
        prior = [("001", "identity", "a")]
        current = prior + [("002", "posts", "b"), ("003", "status", "c")]
        self.assertEqual(upgrade.pending_migrations(prior, current), 2)
        self.assertEqual(
            upgrade.pending_migrations(prior, current + [("004", "future", "d")]), 3
        )

    def test_changed_or_removed_history_is_not_an_upgrade(self):
        prior = [("001", "identity", "a"), ("002", "posts", "b")]
        for current in [
            [],
            prior,
            prior[:1],
            [("001", "identity", "changed"), prior[1], ("003", "status", "c")],
            [("001", "renamed", "a"), prior[1], ("003", "status", "c")],
        ]:
            with self.subTest(current=current), self.assertRaises(AssertionError):
                upgrade.pending_migrations(prior, current)
        with self.assertRaises(AssertionError):
            upgrade.pending_migrations([], prior)

    def test_history_is_read_only_ordered_and_does_not_create_missing_files(self):
        with TemporaryDirectory(prefix="legura-history-") as root:
            path = Path(root) / "contract.db"
            with closing(sqlite3.connect(path)) as db:
                db.execute(
                    "CREATE TABLE _axonyx_migrations "
                    "(version TEXT PRIMARY KEY, name TEXT, checksum TEXT)"
                )
                db.executemany(
                    "INSERT INTO _axonyx_migrations VALUES (?, ?, ?)",
                    [("002", "posts", "b"), ("001", "identity", "a")],
                )
                db.commit()
            before = path.read_bytes()
            self.assertEqual(
                upgrade.migration_history(path),
                [("001", "identity", "a"), ("002", "posts", "b")],
            )
            self.assertEqual(path.read_bytes(), before)
            missing = Path(root) / "missing.db"
            with self.assertRaises(sqlite3.OperationalError):
                upgrade.migration_history(missing)
            self.assertFalse(missing.exists())


if __name__ == "__main__":
    unittest.main()
