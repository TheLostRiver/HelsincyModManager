CREATE TABLE mod_library_projection_state (
    singleton_id        INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    schema_version      INTEGER NOT NULL,
    key_version         TEXT NOT NULL COLLATE BINARY,
    generation          INTEGER NOT NULL CHECK (generation >= 0),
    source_fingerprint  TEXT COLLATE BINARY,
    readiness           TEXT NOT NULL CHECK (readiness IN ('dirty', 'complete')),
    updated_at          INTEGER NOT NULL,
    CHECK (
        readiness = 'dirty'
        OR (readiness = 'complete' AND source_fingerprint IS NOT NULL)
    )
);

INSERT INTO mod_library_projection_state (
    singleton_id,
    schema_version,
    key_version,
    generation,
    source_fingerprint,
    readiness,
    updated_at
) VALUES (
    1,
    1,
    'mod-library-query-key-v1',
    0,
    NULL,
    'dirty',
    CAST(strftime('%s', 'now') AS INTEGER) * 1000
);

CREATE TABLE mod_library_projection_items (
    mod_id               TEXT PRIMARY KEY NOT NULL COLLATE BINARY,
    generation           INTEGER NOT NULL CHECK (generation > 0),
    display_revision_id  TEXT NOT NULL COLLATE BINARY UNIQUE,
    package_id            TEXT NOT NULL COLLATE BINARY UNIQUE,
    display_name          TEXT NOT NULL,
    author                TEXT,
    version_label         TEXT,
    size_label            TEXT NOT NULL,
    preview_image_json    TEXT NOT NULL,
    normalized_name       TEXT NOT NULL COLLATE BINARY,
    normalized_author     TEXT NOT NULL COLLATE BINARY
);

CREATE INDEX idx_mod_library_projection_items_name
    ON mod_library_projection_items(normalized_name COLLATE BINARY, mod_id COLLATE BINARY);

CREATE INDEX idx_mod_library_projection_items_revision
    ON mod_library_projection_items(display_revision_id COLLATE BINARY);

CREATE TABLE mod_library_projection_labels (
    mod_id            TEXT NOT NULL COLLATE BINARY,
    ordinal           INTEGER NOT NULL CHECK (ordinal >= 0),
    category_id       TEXT COLLATE BINARY,
    name              TEXT NOT NULL,
    color             TEXT,
    normalized_name   TEXT NOT NULL COLLATE BINARY,
    PRIMARY KEY (mod_id, ordinal),
    FOREIGN KEY (mod_id) REFERENCES mod_library_projection_items(mod_id) ON DELETE CASCADE
);

CREATE INDEX idx_mod_library_projection_labels_search
    ON mod_library_projection_labels(normalized_name COLLATE BINARY, mod_id COLLATE BINARY);

CREATE INDEX idx_mod_library_projection_labels_category
    ON mod_library_projection_labels(category_id COLLATE BINARY, mod_id COLLATE BINARY)
    WHERE category_id IS NOT NULL;

CREATE TABLE mod_library_projection_profile_generations (
    profile_id          TEXT PRIMARY KEY NOT NULL COLLATE BINARY,
    generation          INTEGER NOT NULL CHECK (generation >= 0),
    source_fingerprint  TEXT COLLATE BINARY,
    readiness           TEXT NOT NULL CHECK (readiness IN ('dirty', 'complete')),
    updated_at          INTEGER NOT NULL,
    UNIQUE (profile_id, generation),
    CHECK (
        readiness = 'dirty'
        OR (readiness = 'complete' AND source_fingerprint IS NOT NULL)
    )
);

CREATE TABLE mod_library_projection_profile_status (
    profile_id           TEXT NOT NULL COLLATE BINARY,
    mod_id               TEXT NOT NULL COLLATE BINARY,
    profile_generation   INTEGER NOT NULL CHECK (profile_generation > 0),
    status               TEXT NOT NULL CHECK (status IN (
        'installed',
        'committed_cleanup_pending',
        'cleanup_pending',
        'rollback_required',
        'repair_required'
    )),
    managed_file_count   INTEGER NOT NULL CHECK (managed_file_count >= 0),
    backup_count         INTEGER NOT NULL CHECK (backup_count >= 0),
    PRIMARY KEY (profile_id, mod_id),
    FOREIGN KEY (profile_id, profile_generation)
        REFERENCES mod_library_projection_profile_generations(profile_id, generation)
        ON DELETE CASCADE,
    FOREIGN KEY (mod_id)
        REFERENCES mod_library_projection_items(mod_id) ON DELETE CASCADE
);

CREATE INDEX idx_mod_library_projection_profile_status_filter
    ON mod_library_projection_profile_status(
        profile_id COLLATE BINARY,
        status COLLATE BINARY,
        mod_id COLLATE BINARY
    );
