-- Preserve existing IDs and restore the parent rows after replacing the table.
PRAGMA defer_foreign_keys = ON;
CREATE TEMP TABLE _legura_users_before_roles AS SELECT id, email, role FROM legura_users;
DROP TABLE legura_users;
CREATE TABLE legura_users (
  id TEXT PRIMARY KEY NOT NULL,
  email TEXT NOT NULL UNIQUE COLLATE NOCASE,
  role TEXT NOT NULL CHECK (role IN ('admin', 'editor', 'subscriber'))
);
INSERT INTO legura_users (id, email, role) SELECT id, email, role FROM _legura_users_before_roles;
DROP TABLE _legura_users_before_roles;

CREATE TRIGGER legura_owner_keep_admin
BEFORE UPDATE OF id, role ON legura_users
WHEN (NEW.id != OLD.id OR NEW.role != 'admin')
  AND EXISTS (SELECT 1 FROM legura_installation WHERE admin_id = OLD.id)
BEGIN
  SELECT RAISE(ABORT, 'installation owner must remain an administrator');
END;

CREATE TRIGGER legura_owner_keep_account
BEFORE DELETE ON legura_users
WHEN EXISTS (SELECT 1 FROM legura_installation WHERE admin_id = OLD.id)
BEGIN
  SELECT RAISE(ABORT, 'installation owner cannot be deleted');
END;
