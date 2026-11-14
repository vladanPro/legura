ALTER TABLE legura_pages ADD COLUMN trashed INTEGER NOT NULL DEFAULT 0 CHECK (trashed IN (0, 1) AND (trashed = 0 OR status = 'draft'));

-- Keep menu references consistent even on connections without foreign_keys enabled.
CREATE TRIGGER legura_menu_items_delete_page
AFTER DELETE ON legura_pages
BEGIN
    DELETE FROM legura_menu_items WHERE page_id = OLD.id;
END;
