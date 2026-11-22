-- An active Legura theme cannot be represented by an older installation.
CREATE TABLE legura_public_theme_previous (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  theme TEXT NOT NULL CHECK (theme IN ('default', 'editorial')),
  color_mode TEXT NOT NULL DEFAULT 'light' CHECK (color_mode IN ('light', 'dark')),
  palette TEXT NOT NULL DEFAULT 'theme' CHECK (palette IN ('theme', 'silver', 'bronze', 'gold'))
);
INSERT INTO legura_public_theme_previous (id, theme, color_mode, palette)
  SELECT id, theme, color_mode, palette FROM legura_public_theme;
DROP TABLE legura_public_theme;
-- Recreate the historical DDL so older maintenance builds can verify it.
CREATE TABLE legura_public_theme (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  theme TEXT NOT NULL CHECK (theme IN ('default', 'editorial'))
);
ALTER TABLE legura_public_theme ADD COLUMN color_mode TEXT NOT NULL DEFAULT 'light' CHECK (color_mode IN ('light', 'dark'));
ALTER TABLE legura_public_theme ADD COLUMN palette TEXT NOT NULL DEFAULT 'theme' CHECK (palette IN ('theme', 'silver', 'bronze', 'gold'));
INSERT INTO legura_public_theme (id, theme, color_mode, palette)
  SELECT id, theme, color_mode, palette FROM legura_public_theme_previous;
DROP TABLE legura_public_theme_previous;
