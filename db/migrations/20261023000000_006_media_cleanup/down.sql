-- Refuse rollback while private bytes may still need deletion.
CREATE TEMP TABLE legura_cleanup_rollback_guard (pending INTEGER CHECK (pending = 0));
INSERT INTO legura_cleanup_rollback_guard
SELECT 1 FROM legura_media_cleanup LIMIT 1;
DROP TABLE legura_cleanup_rollback_guard;

DROP TRIGGER legura_media_cleanup_no_key_reuse;
DROP TABLE legura_media_cleanup;
