CREATE TABLE save_backup_background_settings (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    desired_enabled INTEGER NOT NULL CHECK (desired_enabled IN (0, 1)),
    enabled_at INTEGER NULL,
    last_worker_heartbeat_at INTEGER NULL,
    updated_at INTEGER NOT NULL,
    CHECK (
        (desired_enabled = 0 AND enabled_at IS NULL)
        OR
        (desired_enabled = 1 AND enabled_at IS NOT NULL)
    )
);
