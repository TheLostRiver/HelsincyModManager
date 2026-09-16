use super::*;
use hmm_ports::ModLibrarySort;

#[test]
fn six_sorts_keep_whole_result_order_across_pages_filters_and_unknowns() {
    let temp = tempfile::tempdir().unwrap();
    let conn = Arc::new(Mutex::new(
        crate::open_database(&temp.path().join("hmm.db")).unwrap(),
    ));
    conn.lock().unwrap().execute("INSERT INTO categories (category_id, name, created_at) VALUES ('category-a', 'Armor', 1)", []).unwrap();
    let repository = SqliteModLibraryProjectionRepository::new(Arc::clone(&conn));
    let input = [
        ("a", "Mod 10", Some(20), Some(300)),
        ("b", "Mod 2", Some(10), None),
        ("c", "Mod 1", None, Some(0)),
        ("d", "Mod 02", Some(10), Some(100)),
        ("e", "mod 2", Some(30), Some(100)),
        ("f", "Alpha", None, None),
        ("g", "装备 3", Some(20), Some(100)),
    ];
    let records = input
        .into_iter()
        .map(|(id, name, time, size)| {
            let mut item = record(id, name);
            item.imported_at_unix_millis = time;
            item.content_size_bytes = size;
            if !["b", "d", "e"].contains(&id) {
                item.labels.clear();
            }
            item
        })
        .collect();
    repository
        .rebuild(&ModLibraryProjectionSnapshot {
            source_fingerprint: "sort-catalog".into(),
            records,
            profiles: vec![ModLibraryProfileProjection {
                profile_id: ProfileId::new("scope"),
                source_fingerprint: "sort-status".into(),
                statuses: ["a", "c", "e", "g"]
                    .map(|id| ModLibraryProjectionStatusRecord {
                        mod_id: ModId::new(id),
                        status: ModLibraryProjectionStatus::Installed,
                        managed_file_count: 2,
                        backup_count: 1,
                    })
                    .into(),
            }],
        })
        .unwrap();
    let expected = [
        (
            ModLibrarySort::NameAsc,
            vec!["f", "c", "b", "d", "e", "a", "g"],
        ),
        (
            ModLibrarySort::NameDesc,
            vec!["g", "a", "e", "d", "b", "c", "f"],
        ),
        (
            ModLibrarySort::ImportedAtAsc,
            vec!["b", "d", "a", "g", "e", "f", "c"],
        ),
        (
            ModLibrarySort::ImportedAtDesc,
            vec!["e", "a", "g", "b", "d", "f", "c"],
        ),
        (
            ModLibrarySort::SizeAsc,
            vec!["c", "d", "e", "g", "a", "f", "b"],
        ),
        (
            ModLibrarySort::SizeDesc,
            vec!["a", "d", "e", "g", "c", "f", "b"],
        ),
    ];
    let filters = [
        (
            ModLibraryProjectionQueryFilter::All,
            vec!["a", "b", "c", "d", "e", "f", "g"],
        ),
        (
            ModLibraryProjectionQueryFilter::Category("category-a".into()),
            vec!["b", "d", "e"],
        ),
        (
            ModLibraryProjectionQueryFilter::Status(ModLibraryProjectionQueryStatus::Installed),
            vec!["a", "c", "e", "g"],
        ),
        (
            ModLibraryProjectionQueryFilter::Status(ModLibraryProjectionQueryStatus::NotInstalled),
            vec!["b", "d", "f"],
        ),
    ];
    for (sort, ordered_ids) in expected {
        for (filter, allowed) in &filters {
            for search in ["", "mod"] {
                let expected_ids = ordered_ids
                    .iter()
                    .filter(|id| allowed.contains(id))
                    .filter(|id| search.is_empty() || !["f", "g"].contains(id))
                    .copied()
                    .collect::<Vec<_>>();
                let request = ModLibraryProjectionQueryRequest {
                    source_fingerprint: "sort-catalog".into(),
                    profile: Some(ModLibraryProjectionProfileQuery {
                        profile_id: ProfileId::new("scope"),
                        source_fingerprint: "sort-status".into(),
                    }),
                    normalized_search: search.into(),
                    filter: filter.clone(),
                    sort,
                    page: 1,
                    page_size: 2,
                };
                let mut actual = Vec::new();
                for page in 1..=expected_ids.len().div_ceil(2).max(1) {
                    let result = repository
                        .query(&ModLibraryProjectionQueryRequest {
                            page: page as u64,
                            ..request.clone()
                        })
                        .unwrap();
                    assert_eq!(result.matching_total, expected_ids.len());
                    assert_eq!(result.library_total, 7);
                    for entry in result.items {
                        let id = entry.record.mod_id.as_str();
                        let source = input
                            .iter()
                            .find(|(source_id, _, _, _)| *source_id == id)
                            .unwrap();
                        assert_eq!(entry.record.imported_at_unix_millis, source.2);
                        assert_eq!(entry.record.content_size_bytes, source.3);
                        actual.push(id.to_owned());
                    }
                }
                assert_eq!(actual, expected_ids, "{sort:?} {filter:?} {search:?}");
                let last = repository
                    .query(&ModLibraryProjectionQueryRequest {
                        page: u64::MAX,
                        ..request
                    })
                    .unwrap();
                assert_eq!(last.page, expected_ids.len().div_ceil(2).max(1) as u64);
            }
        }
    }
}
