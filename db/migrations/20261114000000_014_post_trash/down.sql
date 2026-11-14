-- Refuse to silently return trashed posts to the ordinary draft list.
CREATE TABLE _legura_post_trash_rollback_guard (blocked INTEGER CHECK (blocked = 0));
INSERT INTO _legura_post_trash_rollback_guard SELECT count(*) FROM legura_posts WHERE trashed = 1;
DROP TABLE _legura_post_trash_rollback_guard;
ALTER TABLE legura_posts DROP COLUMN trashed;
