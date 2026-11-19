-- Rollback cannot silently discard category names or editorial assignments.
CREATE TABLE _legura_categories_rollback_guard (blocked INTEGER CHECK (blocked = 0));
INSERT INTO _legura_categories_rollback_guard
  SELECT (SELECT count(*) FROM legura_categories) + (SELECT count(*) FROM legura_post_categories);
DROP TABLE _legura_categories_rollback_guard;
DROP TRIGGER legura_post_categories_keep_category_id;
DROP TRIGGER legura_post_categories_keep_post_id;
DROP TRIGGER legura_post_categories_delete_category;
DROP TRIGGER legura_post_categories_delete_post;
DROP TRIGGER legura_post_categories_update_guard;
DROP TRIGGER legura_post_categories_insert_guard;
DROP TABLE legura_post_categories;
DROP TABLE legura_categories;
