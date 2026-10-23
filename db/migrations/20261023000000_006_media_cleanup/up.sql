-- A committed media deletion leaves a durable object reference until its bytes
-- are removed. The key is never reused while cleanup is pending.
CREATE TABLE legura_media_cleanup (
  id TEXT PRIMARY KEY NOT NULL CHECK (length(id) = 64 AND id NOT GLOB '*[^0-9a-f]*'),
  store_id TEXT NOT NULL CHECK (length(store_id) BETWEEN 1 AND 64 AND store_id NOT GLOB '*[^a-z0-9-]*'),
  created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TRIGGER legura_media_cleanup_no_key_reuse BEFORE INSERT ON legura_media
WHEN EXISTS (SELECT 1 FROM legura_media_cleanup WHERE id = NEW.id)
BEGIN
  SELECT RAISE(ABORT, 'Media cleanup is pending for this key');
END;
