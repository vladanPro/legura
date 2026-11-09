CREATE TABLE _legura_pages_rollback_guard (empty INTEGER NOT NULL CHECK (empty = 1));
INSERT INTO _legura_pages_rollback_guard SELECT NOT EXISTS (SELECT 1 FROM legura_pages);
DROP TABLE _legura_pages_rollback_guard;
DROP TABLE legura_pages;
