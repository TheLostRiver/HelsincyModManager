ALTER TABLE profile_save_settings
    ADD COLUMN retention_max_total_bytes INTEGER
        CHECK (retention_max_total_bytes IS NULL
            OR retention_max_total_bytes BETWEEN 16777216 AND 1099511627776);

ALTER TABLE profile_save_settings
    ADD COLUMN steam_account_name TEXT;

ALTER TABLE profile_save_settings
    ADD COLUMN steam_avatar_url TEXT;

ALTER TABLE profile_save_settings
    ADD COLUMN steam_account_label TEXT;

ALTER TABLE save_backups
    ADD COLUMN retention_reasons TEXT;

ALTER TABLE save_backups
    ADD COLUMN retention_attempted_at INTEGER;

ALTER TABLE save_backups
    ADD COLUMN retention_error_code TEXT;

ALTER TABLE save_backups
    ADD COLUMN retention_released_bytes INTEGER NOT NULL DEFAULT 0
        CHECK (retention_released_bytes >= 0);

CREATE INDEX idx_save_backups_center_query
    ON save_backups(game_id, status, trigger, created_at DESC, backup_id DESC);
