ALTER TABLE profile_save_settings
ADD COLUMN pre_restore_backup_enabled INTEGER NOT NULL DEFAULT 1;

CREATE TABLE save_restore_transactions (
    transaction_id          TEXT PRIMARY KEY NOT NULL,
    game_id                TEXT NOT NULL,
    profile_id             TEXT NOT NULL,
    backup_id              TEXT NOT NULL,
    pre_restore_backup_id  TEXT,
    status                 TEXT NOT NULL,
    error_code             TEXT,
    created_at             INTEGER NOT NULL,
    updated_at             INTEGER NOT NULL
);

CREATE INDEX idx_save_restore_transactions_profile_status
    ON save_restore_transactions(game_id, profile_id, status, updated_at DESC);
