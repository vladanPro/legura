-- Downgrading must not silently reactivate disabled accounts.
CREATE TEMP TABLE _legura_active_accounts_only (blocked INTEGER CHECK (blocked = 0));
INSERT INTO _legura_active_accounts_only
  SELECT COUNT(*) FROM legura_users WHERE status = 'inactive';
DROP TABLE _legura_active_accounts_only;

DROP TRIGGER legura_account_session_guard;
DROP TRIGGER legura_account_status_revoke_sessions;
DROP TRIGGER legura_account_status_guard;
ALTER TABLE legura_users DROP COLUMN session_generation;
ALTER TABLE legura_users DROP COLUMN status;
