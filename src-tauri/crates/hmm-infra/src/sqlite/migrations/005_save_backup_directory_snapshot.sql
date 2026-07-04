ALTER TABLE save_backups
    ADD COLUMN backup_directory_mode TEXT NOT NULL DEFAULT 'default';

ALTER TABLE save_backups
    ADD COLUMN backup_directory TEXT;
