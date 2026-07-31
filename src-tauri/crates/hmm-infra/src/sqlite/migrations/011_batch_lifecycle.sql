CREATE TABLE hmm_batch_lifecycle_batches (
    batch_id TEXT PRIMARY KEY NOT NULL COLLATE BINARY,
    sealed_json TEXT NOT NULL,
    created_at_unix_millis INTEGER NOT NULL
);

CREATE TABLE hmm_batch_lifecycle_attempts (
    batch_id TEXT NOT NULL COLLATE BINARY,
    attempt_number INTEGER NOT NULL,
    attempt_json TEXT NOT NULL,
    PRIMARY KEY (batch_id, attempt_number),
    FOREIGN KEY (batch_id) REFERENCES hmm_batch_lifecycle_batches(batch_id) ON DELETE CASCADE
);

CREATE TABLE hmm_batch_lifecycle_item_results (
    batch_id TEXT NOT NULL COLLATE BINARY,
    attempt_number INTEGER NOT NULL,
    item_id TEXT NOT NULL COLLATE BINARY,
    ordinal INTEGER NOT NULL,
    result_json TEXT NOT NULL,
    PRIMARY KEY (batch_id, attempt_number, item_id),
    UNIQUE (batch_id, attempt_number, ordinal),
    FOREIGN KEY (batch_id, attempt_number)
        REFERENCES hmm_batch_lifecycle_attempts(batch_id, attempt_number)
        ON DELETE CASCADE
);

CREATE INDEX hmm_batch_lifecycle_item_results_order
    ON hmm_batch_lifecycle_item_results(batch_id, attempt_number, ordinal ASC);
