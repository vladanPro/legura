CREATE TABLE legura_public_theme_next (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  theme TEXT NOT NULL CHECK (theme IN ('default', 'editorial', 'legura')),
  color_mode TEXT NOT NULL DEFAULT 'light' CHECK (color_mode IN ('light', 'dark')),
  palette TEXT NOT NULL DEFAULT 'theme' CHECK (palette IN ('theme', 'silver', 'bronze', 'gold'))
);
INSERT INTO legura_public_theme_next (id, theme, color_mode, palette)
  SELECT id, theme, color_mode, palette FROM legura_public_theme;
DROP TABLE legura_public_theme;
ALTER TABLE legura_public_theme_next RENAME TO legura_public_theme;
