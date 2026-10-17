-- The migration executor runs this transactionally. Never discard media references.
CREATE TABLE _legura_media_rollback_guard (blocked INTEGER CHECK (blocked = 0));
INSERT INTO _legura_media_rollback_guard SELECT count(*) FROM legura_media;
DROP TABLE _legura_media_rollback_guard;
DROP TABLE legura_media;
