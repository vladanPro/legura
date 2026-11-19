CREATE TABLE legura_categories (
  id TEXT PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128 AND instr(id, char(0)) = 0),
  name TEXT NOT NULL CHECK (
    length(name) BETWEEN 1 AND 80 AND name = trim(name) AND
    instr(name, char(0)) = 0 AND instr(name, char(10)) = 0 AND instr(name, char(13)) = 0
  ),
  slug TEXT NOT NULL UNIQUE CHECK (
    length(slug) BETWEEN 1 AND 120 AND instr(slug, char(0)) = 0 AND
    slug NOT GLOB '*[^a-z0-9-]*' AND
    substr(slug, 1, 1) GLOB '[a-z0-9]' AND substr(slug, -1) GLOB '[a-z0-9]'
  )
);

CREATE TABLE legura_post_categories (
  post_id TEXT NOT NULL REFERENCES legura_posts(id) ON DELETE CASCADE,
  category_id TEXT NOT NULL REFERENCES legura_categories(id) ON DELETE CASCADE,
  PRIMARY KEY (post_id, category_id)
);
CREATE INDEX legura_post_categories_category_idx ON legura_post_categories(category_id, post_id);

-- Preserve integrity even on framework connections with foreign_keys disabled.
CREATE TRIGGER legura_post_categories_insert_guard BEFORE INSERT ON legura_post_categories
WHEN NOT EXISTS (SELECT 1 FROM legura_posts WHERE id = NEW.post_id) OR
     NOT EXISTS (SELECT 1 FROM legura_categories WHERE id = NEW.category_id)
BEGIN
  SELECT RAISE(ABORT, 'Unknown post or category');
END;
CREATE TRIGGER legura_post_categories_update_guard BEFORE UPDATE ON legura_post_categories
WHEN NOT EXISTS (SELECT 1 FROM legura_posts WHERE id = NEW.post_id) OR
     NOT EXISTS (SELECT 1 FROM legura_categories WHERE id = NEW.category_id)
BEGIN
  SELECT RAISE(ABORT, 'Unknown post or category');
END;
CREATE TRIGGER legura_post_categories_delete_post AFTER DELETE ON legura_posts
BEGIN
  DELETE FROM legura_post_categories WHERE post_id = OLD.id;
END;
CREATE TRIGGER legura_post_categories_delete_category AFTER DELETE ON legura_categories
BEGIN
  DELETE FROM legura_post_categories WHERE category_id = OLD.id;
END;
CREATE TRIGGER legura_post_categories_keep_post_id BEFORE UPDATE OF id ON legura_posts
WHEN NEW.id != OLD.id AND EXISTS (SELECT 1 FROM legura_post_categories WHERE post_id = OLD.id)
BEGIN
  SELECT RAISE(ABORT, 'Cannot change a categorized post identity');
END;
CREATE TRIGGER legura_post_categories_keep_category_id BEFORE UPDATE OF id ON legura_categories
WHEN NEW.id != OLD.id AND EXISTS (SELECT 1 FROM legura_post_categories WHERE category_id = OLD.id)
BEGIN
  SELECT RAISE(ABORT, 'Cannot change an attached category identity');
END;
