ALTER TABLE legura_users ADD COLUMN status TEXT NOT NULL DEFAULT 'active'
  CHECK (status IN ('active', 'inactive'));
ALTER TABLE legura_users ADD COLUMN session_generation TEXT NOT NULL DEFAULT '';

-- Match the runtime store so migrations also work on a new, unopened database.
CREATE TABLE IF NOT EXISTS ax_sessions (
  id TEXT PRIMARY KEY NOT NULL,
  subject TEXT NOT NULL,
  data_json TEXT NOT NULL,
  created_at_unix INTEGER NOT NULL,
  last_seen_at_unix INTEGER NOT NULL,
  expires_at_unix INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS ax_sessions_expires_at_idx ON ax_sessions (expires_at_unix);

CREATE TRIGGER legura_account_status_guard
BEFORE UPDATE OF status, session_generation ON legura_users
WHEN NEW.status != OLD.status AND (
  OLD.role = 'admin' OR NEW.role = 'admin' OR
  EXISTS (SELECT 1 FROM legura_installation WHERE admin_id = OLD.id) OR
  NEW.session_generation = '' OR NEW.session_generation = OLD.session_generation
)
BEGIN
  SELECT RAISE(ABORT, 'account status transition is not permitted');
END;

CREATE TRIGGER legura_account_status_revoke_sessions
AFTER UPDATE OF session_generation ON legura_users
WHEN NEW.session_generation != OLD.session_generation
BEGIN
  DELETE FROM ax_sessions WHERE subject = OLD.id;
END;

-- A sign-in started before a status transition cannot save a session afterward.
CREATE TRIGGER legura_account_session_guard
BEFORE INSERT ON ax_sessions
WHEN NOT EXISTS (
  SELECT 1 FROM legura_users WHERE id = NEW.subject AND status = 'active'
    AND session_generation = COALESCE(json_extract(NEW.data_json, '$.legura_generation'), '')
)
BEGIN
  SELECT RAISE(ABORT, 'session is no longer eligible');
END;
