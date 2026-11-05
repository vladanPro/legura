CREATE TABLE legura_public_theme (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  theme TEXT NOT NULL CHECK (theme IN ('default', 'editorial'))
);
INSERT INTO legura_public_theme (id, theme) VALUES (1, 'default');
