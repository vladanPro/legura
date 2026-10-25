-- The migration executor runs this transactionally. Never flatten rich posts.
CREATE TABLE _legura_richtext_rollback_guard (blocked INTEGER CHECK (blocked = 0));
INSERT INTO _legura_richtext_rollback_guard
  SELECT count(*) FROM legura_posts
  WHERE content_format != 'plain-text' OR content_document IS NOT NULL;
DROP TABLE _legura_richtext_rollback_guard;
ALTER TABLE legura_posts DROP COLUMN content_document;
ALTER TABLE legura_posts DROP COLUMN content_format;
