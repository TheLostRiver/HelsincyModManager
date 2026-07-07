CREATE TABLE IF NOT EXISTS save_backup_scheduler_state (
    game_id TEXT NOT NULL,
    profile_id TEXT NOT NULL,
    enabled INTEGER NOT NULL,
    background_protection_enabled INTEGER NOT NULL,
    background_status TEXT NOT NULL,
    last_checked_at INTEGER,
    last_attempt_at INTEGER,
    last_success_at INTEGER,
    next_due_at INTEGER,
    pending_reason TEXT,
    last_error_code TEXT,
    worker_instance_id TEXT,
    lease_owner TEXT,
    lease_expires_at INTEGER,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (game_id, profile_id)
);
