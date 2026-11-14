ALTER TABLE legura_posts ADD COLUMN created_by TEXT REFERENCES legura_users(id);
ALTER TABLE legura_posts ADD COLUMN created_at TEXT;

-- Existing posts have no reliable author or creation time. Only stamp new
-- posts whose creation was attributed to a signed-in administrator.
CREATE TRIGGER legura_posts_record_creation
AFTER INSERT ON legura_posts
WHEN NEW.created_by IS NOT NULL AND NEW.created_at IS NULL
BEGIN
    UPDATE legura_posts
    SET created_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
    WHERE id = NEW.id;
END;
