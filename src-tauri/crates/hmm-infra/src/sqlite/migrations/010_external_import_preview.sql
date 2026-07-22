CREATE TABLE external_import_batches (
    batch_id   TEXT PRIMARY KEY NOT NULL COLLATE BINARY,
    batch_json TEXT NOT NULL
);

CREATE TABLE external_import_candidates (
    batch_id       TEXT NOT NULL COLLATE BINARY,
    candidate_id   TEXT NOT NULL COLLATE BINARY,
    ordinal        INTEGER NOT NULL CHECK (ordinal >= 0),
    candidate_json TEXT NOT NULL,
    PRIMARY KEY (batch_id, candidate_id),
    UNIQUE (batch_id, ordinal),
    FOREIGN KEY (batch_id) REFERENCES external_import_batches(batch_id) ON DELETE CASCADE
);

CREATE INDEX idx_external_import_candidates_page
    ON external_import_candidates(batch_id COLLATE BINARY, ordinal ASC);

CREATE TABLE external_import_selections (
    selection_id   TEXT PRIMARY KEY NOT NULL COLLATE BINARY,
    batch_id       TEXT NOT NULL COLLATE BINARY,
    selection_json TEXT NOT NULL,
    FOREIGN KEY (batch_id) REFERENCES external_import_batches(batch_id) ON DELETE CASCADE
);

CREATE INDEX idx_external_import_selections_batch
    ON external_import_selections(batch_id COLLATE BINARY);

CREATE TABLE external_import_item_results (
    batch_id    TEXT NOT NULL COLLATE BINARY,
    candidate_id TEXT NOT NULL COLLATE BINARY,
    ordinal      INTEGER NOT NULL CHECK (ordinal >= 0),
    result_json  TEXT NOT NULL,
    PRIMARY KEY (batch_id, candidate_id),
    FOREIGN KEY (batch_id) REFERENCES external_import_batches(batch_id) ON DELETE CASCADE
);

CREATE INDEX idx_external_import_item_results_page
    ON external_import_item_results(batch_id COLLATE BINARY, ordinal ASC);
