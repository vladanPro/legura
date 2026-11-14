CREATE TABLE legura_general_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  tagline TEXT NOT NULL DEFAULT '' CHECK (length(tagline) <= 160),
  site_icon TEXT NOT NULL DEFAULT '',
  legura_url TEXT NOT NULL DEFAULT '',
  site_address TEXT NOT NULL DEFAULT '',
  admin_email TEXT NOT NULL DEFAULT '',
  timezone TEXT NOT NULL DEFAULT 'UTC',
  date_format TEXT NOT NULL DEFAULT 'yyyy-mm-dd' CHECK (date_format IN ('yyyy-mm-dd', 'dd/mm/yyyy', 'mm/dd/yyyy')),
  time_format TEXT NOT NULL DEFAULT '24h' CHECK (time_format IN ('24h', '12h')),
  week_start INTEGER NOT NULL DEFAULT 1 CHECK (week_start BETWEEN 0 AND 6)
);
INSERT INTO legura_general_settings (id) VALUES (1);
