CREATE TABLE legura_posts (
  id TEXT PRIMARY KEY NOT NULL,
  title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 160),
  slug TEXT NOT NULL UNIQUE CHECK (
    length(slug) BETWEEN 1 AND 120
    AND slug NOT GLOB '*[^a-z0-9-]*'
    AND substr(slug, 1, 1) != '-'
    AND substr(slug, -1, 1) != '-'
  ),
  body TEXT NOT NULL CHECK (length(trim(body)) BETWEEN 1 AND 20000),
  status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'published'))
);
