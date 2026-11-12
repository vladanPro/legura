ALTER TABLE legura_public_theme ADD COLUMN palette TEXT NOT NULL DEFAULT 'theme' CHECK (palette IN ('theme', 'silver', 'bronze', 'gold'));
