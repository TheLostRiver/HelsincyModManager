CREATE TABLE mod_metadata (
    mod_id        TEXT    PRIMARY KEY NOT NULL,
    display_name  TEXT,
    author        TEXT,
    version       TEXT,
    description   TEXT,
    nexus_mod_id  INTEGER,
    updated_at    INTEGER NOT NULL  -- unix millis
);

CREATE TABLE categories (
    category_id   TEXT    PRIMARY KEY NOT NULL,
    name          TEXT    NOT NULL,
    color         TEXT,                         -- hex color (e.g. "#FF6B6B")
    sort_order    INTEGER NOT NULL DEFAULT 0,
    created_at    INTEGER NOT NULL              -- unix millis
);

CREATE TABLE mod_categories (
    mod_id        TEXT NOT NULL,
    category_id   TEXT NOT NULL,
    PRIMARY KEY (mod_id, category_id),
    FOREIGN KEY (category_id) REFERENCES categories(category_id) ON DELETE CASCADE
);

CREATE INDEX idx_mod_categories_category ON mod_categories(category_id);
