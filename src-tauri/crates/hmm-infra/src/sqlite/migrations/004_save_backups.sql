CREATE TABLE save_backups (
    backup_id           TEXT PRIMARY KEY,
    game_id             TEXT NOT NULL,
    profile_id          TEXT NOT NULL,
    trigger             TEXT NOT NULL,
    status              TEXT NOT NULL,
    archive_file_name   TEXT NOT NULL,
    manifest_file_name  TEXT NOT NULL,
    archive_size_bytes  INTEGER NOT NULL,
    archive_sha256      TEXT NOT NULL,
    file_count          INTEGER NOT NULL,
    created_at          INTEGER NOT NULL,
    source_path_label   TEXT,
    source_path_hash    TEXT NOT NULL,
    notes               TEXT
);

CREATE INDEX idx_save_backups_profile_created
    ON save_backups(game_id, profile_id, created_at DESC);
