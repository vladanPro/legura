-- Refuse to silently return trashed pages to the ordinary draft list.
CREATE TABLE _legura_page_trash_rollback_guard (blocked INTEGER CHECK (blocked = 0));
INSERT INTO _legura_page_trash_rollback_guard SELECT count(*) FROM legura_pages WHERE trashed = 1;
DROP TABLE _legura_page_trash_rollback_guard;
DROP TRIGGER legura_menu_items_delete_page;
ALTER TABLE legura_pages DROP COLUMN trashed;
