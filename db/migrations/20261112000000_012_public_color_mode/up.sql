ALTER TABLE legura_public_theme ADD COLUMN color_mode TEXT NOT NULL DEFAULT 'light' CHECK (color_mode IN ('light', 'dark'));
