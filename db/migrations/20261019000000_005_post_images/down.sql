-- Transactional rollback must not silently discard editorial image selections.
CREATE TABLE _legura_post_images_rollback_guard (blocked INTEGER CHECK (blocked = 0));
INSERT INTO _legura_post_images_rollback_guard SELECT count(*) FROM legura_post_images;
DROP TABLE _legura_post_images_rollback_guard;
DROP TRIGGER legura_post_images_delete_post;
DROP TRIGGER legura_post_images_keep_media;
DROP TABLE legura_post_images;
