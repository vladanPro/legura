CREATE TABLE legura_languages (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  admin_language TEXT NOT NULL CHECK (admin_language IN ('en', 'sr-Latn')),
  site_language TEXT NOT NULL CHECK (site_language IN ('en', 'sr-Latn'))
);
INSERT INTO legura_languages (id, admin_language, site_language) VALUES (1, 'en', 'en');
