-- Refuse a downgrade that would silently discard attribution.
CREATE TABLE _legura_post_creation_rollback_guard (blocked INTEGER CHECK (blocked = 0));
INSERT INTO _legura_post_creation_rollback_guard
SELECT count(*) FROM legura_posts WHERE created_by IS NOT NULL OR created_at IS NOT NULL;
DROP TABLE _legura_post_creation_rollback_guard;
DROP TRIGGER legura_posts_record_creation;
ALTER TABLE legura_posts DROP COLUMN created_at;
ALTER TABLE legura_posts DROP COLUMN created_by;
