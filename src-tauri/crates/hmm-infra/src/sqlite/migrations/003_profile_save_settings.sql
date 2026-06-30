CREATE TABLE profile_save_settings (
    profile_id              TEXT    PRIMARY KEY NOT NULL,
    save_directory          TEXT,
    backup_directory        TEXT,
    backup_cadence          TEXT    NOT NULL DEFAULT 'manual',
    backup_hour             INTEGER,
    backup_minute           INTEGER,
    backup_weekdays         TEXT    NOT NULL DEFAULT '[]',
    retention_max_count     INTEGER NOT NULL DEFAULT 20,
    retention_max_age_days  INTEGER,
    updated_at              INTEGER NOT NULL,
    FOREIGN KEY(profile_id) REFERENCES profiles(profile_id) ON DELETE CASCADE
);
