CREATE TABLE legura_post_images (
  post_id TEXT PRIMARY KEY NOT NULL REFERENCES legura_posts(id) ON DELETE CASCADE,
  media_id TEXT NOT NULL REFERENCES legura_media(id) ON DELETE RESTRICT,
  alt TEXT NOT NULL CHECK (length(alt) <= 300 AND instr(alt, char(0)) = 0)
);
CREATE INDEX legura_post_images_media_idx ON legura_post_images(media_id);

-- Framework connections do not all enable SQLite foreign_keys. Preserve deletion
-- semantics on those connections too; the write service enables FK checks itself.
CREATE TRIGGER legura_post_images_delete_post AFTER DELETE ON legura_posts
BEGIN
  DELETE FROM legura_post_images WHERE post_id = OLD.id;
END;
CREATE TRIGGER legura_post_images_keep_media BEFORE DELETE ON legura_media
WHEN EXISTS (SELECT 1 FROM legura_post_images WHERE media_id = OLD.id)
BEGIN
  SELECT RAISE(ABORT, 'Media is attached to a post');
END;
