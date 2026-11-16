-- Do not silently promote, remove, or rewrite non-administrator accounts.
CREATE TEMP TABLE _legura_roles_rollback_guard (blocked INTEGER CHECK (blocked = 0));
INSERT INTO _legura_roles_rollback_guard SELECT count(*) FROM legura_users WHERE role != 'admin';
DROP TABLE _legura_roles_rollback_guard;

PRAGMA defer_foreign_keys = ON;
DROP TRIGGER legura_owner_keep_account;
DROP TRIGGER legura_owner_keep_admin;
CREATE TEMP TABLE _legura_users_before_roles AS SELECT id, email, role FROM legura_users;
DROP TABLE legura_users;
CREATE TABLE legura_users (
  id TEXT PRIMARY KEY NOT NULL,
  email TEXT NOT NULL UNIQUE COLLATE NOCASE,
  role TEXT NOT NULL CHECK (role = 'admin')
);
INSERT INTO legura_users (id, email, role) SELECT id, email, role FROM _legura_users_before_roles;
DROP TABLE _legura_users_before_roles;
