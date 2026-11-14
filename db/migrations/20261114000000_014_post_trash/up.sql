ALTER TABLE legura_posts ADD COLUMN trashed INTEGER NOT NULL DEFAULT 0 CHECK (trashed IN (0, 1) AND (trashed = 0 OR status = 'draft'));
