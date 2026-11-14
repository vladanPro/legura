CREATE TABLE _legura_general_settings_rollback_guard (blocked INTEGER CHECK (blocked = 0));
INSERT INTO _legura_general_settings_rollback_guard
SELECT count(*) FROM legura_general_settings
WHERE tagline != '' OR site_icon != '' OR legura_url != '' OR site_address != ''
   OR admin_email != '' OR timezone != 'UTC' OR date_format != 'yyyy-mm-dd'
   OR time_format != '24h' OR week_start != 1;
DROP TABLE _legura_general_settings_rollback_guard;
DROP TABLE legura_general_settings;
