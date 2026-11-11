CREATE TABLE legura_menus (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(trim(name)) BETWEEN 1 AND 80)
);

CREATE TABLE legura_menu_items (
  menu_id TEXT NOT NULL REFERENCES legura_menus(id) ON DELETE CASCADE,
  page_id TEXT NOT NULL REFERENCES legura_pages(id) ON DELETE CASCADE,
  position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 10000),
  visible INTEGER NOT NULL DEFAULT 1 CHECK (visible IN (0, 1)),
  PRIMARY KEY (menu_id, page_id)
);

CREATE TABLE legura_menu_locations (
  location TEXT PRIMARY KEY NOT NULL CHECK (location IN ('header', 'footer')),
  menu_id TEXT REFERENCES legura_menus(id) ON DELETE SET NULL
);

INSERT INTO legura_menu_locations (location) VALUES ('header'), ('footer');
