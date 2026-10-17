CREATE TABLE legura_media (
  id TEXT PRIMARY KEY NOT NULL CHECK (length(id) = 64 AND id NOT GLOB '*[^0-9a-f]*'),
  store_id TEXT NOT NULL CHECK (length(store_id) BETWEEN 1 AND 64 AND store_id NOT GLOB '*[^a-z0-9-]*'),
  content_type TEXT NOT NULL CHECK (content_type IN ('image/jpeg', 'image/png')),
  byte_size INTEGER NOT NULL CHECK (byte_size BETWEEN 1 AND 8388608),
  width INTEGER NOT NULL CHECK (width BETWEEN 1 AND 8192),
  height INTEGER NOT NULL CHECK (height BETWEEN 1 AND 8192 AND width * height <= 16000000),
  sha256 TEXT NOT NULL CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*')
);
