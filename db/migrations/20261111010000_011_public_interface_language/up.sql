ALTER TABLE legura_languages ADD COLUMN interface_language TEXT NOT NULL DEFAULT 'en' CHECK (interface_language IN ('en', 'fr'));
ALTER TABLE legura_languages ADD COLUMN interface_switcher INTEGER NOT NULL DEFAULT 0 CHECK (interface_switcher IN (0, 1));
