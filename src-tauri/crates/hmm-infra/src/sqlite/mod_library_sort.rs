use hmm_ports::ModLibrarySort;

/// SQL fragments come only from this closed enum, never from request text. The matching
/// expression indexes keep unknown values last in both directions without a temporary sort.
pub(super) fn sort_sql(sort: ModLibrarySort) -> (&'static str, &'static str) {
    match sort {
        ModLibrarySort::NameAsc => ("idx_mod_library_projection_items_name", "i.name_sort_key ASC, i.mod_id COLLATE BINARY ASC"),
        ModLibrarySort::NameDesc => ("idx_mod_library_projection_items_name", "i.name_sort_key DESC, i.mod_id COLLATE BINARY DESC"),
        ModLibrarySort::ImportedAtAsc => ("idx_mod_library_projection_items_imported_asc", "i.imported_at_unix_millis IS NULL, i.imported_at_unix_millis ASC, i.name_sort_key, i.mod_id COLLATE BINARY"),
        ModLibrarySort::ImportedAtDesc => ("idx_mod_library_projection_items_imported_desc", "i.imported_at_unix_millis IS NULL, i.imported_at_unix_millis DESC, i.name_sort_key, i.mod_id COLLATE BINARY"),
        ModLibrarySort::SizeAsc => ("idx_mod_library_projection_items_size_asc", "i.content_size_bytes IS NULL, i.content_size_bytes ASC, i.name_sort_key, i.mod_id COLLATE BINARY"),
        ModLibrarySort::SizeDesc => ("idx_mod_library_projection_items_size_desc", "i.content_size_bytes IS NULL, i.content_size_bytes DESC, i.name_sort_key, i.mod_id COLLATE BINARY"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sort_and_stored_status_path_uses_its_index_without_temporary_ordering() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        super::super::migrations::migrations()
            .to_latest(&mut conn)
            .unwrap();
        for sort in [
            ModLibrarySort::NameAsc,
            ModLibrarySort::NameDesc,
            ModLibrarySort::ImportedAtAsc,
            ModLibrarySort::ImportedAtDesc,
            ModLibrarySort::SizeAsc,
            ModLibrarySort::SizeDesc,
        ] {
            let (index, order) = sort_sql(sort);
            let queries = [
                format!("SELECT i.mod_id FROM mod_library_projection_items i INDEXED BY {index} WHERE i.generation = 1 ORDER BY {order} LIMIT 24"),
                super::super::mod_library_projection_repository::stored_status_rows_sql(sort),
            ];
            for sql in queries {
                let mut statement = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
                let values = rusqlite::params![1, "scope", 1, "installed", "", 24, 0];
                let params = if statement.parameter_count() > 0 {
                    values
                } else {
                    &[]
                };
                let plan = statement
                    .query_map(params, |row| row.get::<_, String>(3))
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap()
                    .join("\n");
                assert!(plan.contains(index), "{sort:?}: {plan}");
                assert!(!plan.contains("TEMP B-TREE"), "{sort:?}: {plan}");
            }
        }
    }
}
