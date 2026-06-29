CREATE TABLE profiles (
    profile_id   TEXT    PRIMARY KEY NOT NULL,
    name         TEXT    NOT NULL,
    description  TEXT,
    is_active    INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);

CREATE UNIQUE INDEX idx_profiles_single_active
    ON profiles(is_active)
    WHERE is_active = 1;

INSERT OR IGNORE INTO profiles
    (profile_id, name, description, is_active, created_at, updated_at)
VALUES
    ('default', 'Default', NULL, 1, 0, 0);
