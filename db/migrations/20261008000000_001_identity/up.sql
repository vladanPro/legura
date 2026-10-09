CREATE TABLE legura_users (
  id TEXT PRIMARY KEY NOT NULL,
  email TEXT NOT NULL UNIQUE COLLATE NOCASE,
  role TEXT NOT NULL CHECK (role = 'admin')
);
CREATE TABLE legura_credentials (
  user_id TEXT PRIMARY KEY NOT NULL REFERENCES legura_users(id),
  email TEXT NOT NULL UNIQUE COLLATE NOCASE,
  password_hash TEXT NOT NULL
);
-- A singleton primary key serializes competing setup transactions.
CREATE TABLE legura_installation (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  site_name TEXT NOT NULL,
  admin_id TEXT NOT NULL REFERENCES legura_users(id)
);
