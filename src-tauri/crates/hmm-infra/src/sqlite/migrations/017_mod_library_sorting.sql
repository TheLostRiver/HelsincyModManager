ALTER TABLE mod_library_projection_items ADD COLUMN imported_at_unix_millis INTEGER
    CHECK (imported_at_unix_millis IS NULL OR imported_at_unix_millis >= 0);
ALTER TABLE mod_library_projection_items ADD COLUMN content_size_bytes INTEGER
    CHECK (content_size_bytes IS NULL OR content_size_bytes >= 0);
ALTER TABLE mod_library_projection_items ADD COLUMN name_sort_key BLOB NOT NULL DEFAULT X'';

DROP INDEX idx_mod_library_projection_items_name;
CREATE INDEX idx_mod_library_projection_items_name
    ON mod_library_projection_items(generation, name_sort_key, mod_id COLLATE BINARY);
CREATE INDEX idx_mod_library_projection_items_imported_asc
    ON mod_library_projection_items(generation, imported_at_unix_millis IS NULL,
        imported_at_unix_millis ASC, name_sort_key, mod_id COLLATE BINARY);
CREATE INDEX idx_mod_library_projection_items_imported_desc
    ON mod_library_projection_items(generation, imported_at_unix_millis IS NULL,
        imported_at_unix_millis DESC, name_sort_key, mod_id COLLATE BINARY);
CREATE INDEX idx_mod_library_projection_items_size_asc
    ON mod_library_projection_items(generation, content_size_bytes IS NULL,
        content_size_bytes ASC, name_sort_key, mod_id COLLATE BINARY);
CREATE INDEX idx_mod_library_projection_items_size_desc
    ON mod_library_projection_items(generation, content_size_bytes IS NULL,
        content_size_bytes DESC, name_sort_key, mod_id COLLATE BINARY);

UPDATE mod_library_projection_state SET readiness = 'dirty';
