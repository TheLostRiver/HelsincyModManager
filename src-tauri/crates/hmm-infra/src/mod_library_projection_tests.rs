use crate::SqliteModLibraryProjectionRepository;
use hmm_core::{ModId, ModRevisionId, ProfileId};
use hmm_ports::{
    ModLibraryProfileProjection, ModLibraryProjectionLabel, ModLibraryProjectionProfileQuery,
    ModLibraryProjectionQueryError, ModLibraryProjectionQueryFilter,
    ModLibraryProjectionQueryRepository, ModLibraryProjectionQueryRequest,
    ModLibraryProjectionQueryStatus, ModLibraryProjectionReadiness, ModLibraryProjectionRecord,
    ModLibraryProjectionRepository, ModLibraryProjectionSnapshot, ModLibraryProjectionStatus,
    ModLibraryProjectionStatusRecord, StoredImportPreviewImage, MOD_LIBRARY_QUERY_KEY_VERSION,
};
use std::sync::{Arc, Mutex};

#[test]
fn projection_migration_declares_expected_tables_keys_and_binary_indexes() {
    let temp = tempfile::tempdir().expect("temporary app data");
    let conn = crate::open_database(&temp.path().join("hmm.db")).expect("open database");

    for table in [
        "mod_library_projection_state",
        "mod_library_projection_items",
        "mod_library_projection_labels",
        "mod_library_projection_profile_generations",
        "mod_library_projection_profile_status",
    ] {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .expect("query projection table");
        assert_eq!(count, 1, "missing projection table {table}");
    }

    let key_version: String = conn
        .query_row(
            "SELECT key_version FROM mod_library_projection_state WHERE singleton_id = 1",
            [],
            |row| row.get(0),
        )
        .expect("read projection key version");
    assert_eq!(key_version, MOD_LIBRARY_QUERY_KEY_VERSION);

    for index in [
        "idx_mod_library_projection_items_name",
        "idx_mod_library_projection_items_revision",
        "idx_mod_library_projection_labels_search",
        "idx_mod_library_projection_labels_category",
        "idx_mod_library_projection_profile_status_filter",
    ] {
        let pragma = format!("PRAGMA index_xinfo('{index}')");
        let mut statement = conn.prepare(&pragma).expect("prepare index metadata query");
        let collations = statement
            .query_map([], |row| {
                Ok((row.get::<_, i64>(5)?, row.get::<_, String>(4)?))
            })
            .expect("query index metadata")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect index metadata")
            .into_iter()
            .filter_map(|(is_key, collation)| (is_key == 1).then_some(collation))
            .collect::<Vec<_>>();
        assert!(!collations.is_empty(), "index {index} has no key columns");
        assert!(
            collations.iter().all(|collation| collation == "BINARY"),
            "index {index} must use BINARY collation: {collations:?}"
        );
    }

    let mut statement = conn
        .prepare("PRAGMA foreign_key_list('mod_library_projection_profile_status')")
        .expect("prepare profile status foreign keys");
    let foreign_keys = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .expect("query profile status foreign keys")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect profile status foreign keys");
    let generation_fk = foreign_keys
        .iter()
        .find(|(_, table, from, to)| {
            table == "mod_library_projection_profile_generations"
                && from == "profile_generation"
                && to == "generation"
        })
        .map(|(id, _, _, _)| *id)
        .expect("profile generation foreign key");
    assert!(foreign_keys.iter().any(|(id, table, from, to)| {
        *id == generation_fk
            && table == "mod_library_projection_profile_generations"
            && from == "profile_id"
            && to == "profile_id"
    }));
}

#[test]
fn projection_migration_rebuild_and_profile_completeness_are_atomic_contracts() {
    let temp = tempfile::tempdir().expect("temporary app data");
    let conn = crate::open_database(&temp.path().join("hmm.db")).expect("open database");
    let conn = Arc::new(Mutex::new(conn));
    let repository = SqliteModLibraryProjectionRepository::new(Arc::clone(&conn));

    let initial = repository.state().expect("read initial projection state");
    assert_eq!(initial.generation, 0);
    assert_eq!(initial.readiness, ModLibraryProjectionReadiness::Dirty);
    assert_eq!(initial.source_fingerprint, None);

    let profile_id = ProfileId::new("profile-a");
    let snapshot = ModLibraryProjectionSnapshot {
        source_fingerprint: "catalog-fingerprint-v1".to_owned(),
        records: vec![record("mod-a", "Ａrmor\u{3000}CAFÉ")],
        profiles: vec![ModLibraryProfileProjection {
            profile_id: profile_id.clone(),
            source_fingerprint: "manifest-fingerprint-v1".to_owned(),
            statuses: vec![ModLibraryProjectionStatusRecord {
                mod_id: ModId::new("mod-a"),
                status: ModLibraryProjectionStatus::Installed,
                managed_file_count: 2,
                backup_count: 1,
            }],
        }],
    };

    let published = repository.rebuild(&snapshot).expect("publish projection");
    assert_eq!(published.generation, 1);
    assert!(published.is_complete_for("catalog-fingerprint-v1"));

    let profile_state = repository
        .profile_state(&profile_id)
        .expect("read profile state")
        .expect("profile state exists");
    assert!(profile_state.is_complete_for("manifest-fingerprint-v1"));

    let conn_guard = conn.lock().expect("database lock");
    let item_count: i64 = conn_guard
        .query_row(
            "SELECT COUNT(*) FROM mod_library_projection_items",
            [],
            |row| row.get(0),
        )
        .expect("count projection items");
    let label_count: i64 = conn_guard
        .query_row(
            "SELECT COUNT(*) FROM mod_library_projection_labels",
            [],
            |row| row.get(0),
        )
        .expect("count projection labels");
    let status_count: i64 = conn_guard
        .query_row(
            "SELECT COUNT(*) FROM mod_library_projection_profile_status",
            [],
            |row| row.get(0),
        )
        .expect("count profile status rows");
    let normalized_name: String = conn_guard
        .query_row(
            "SELECT normalized_name FROM mod_library_projection_items WHERE mod_id = 'mod-a'",
            [],
            |row| row.get(0),
        )
        .expect("read normalized query key");
    assert_eq!(item_count, 1);
    assert_eq!(label_count, 1);
    assert_eq!(status_count, 1);
    assert_eq!(normalized_name, "armor café");
    drop(conn_guard);

    repository
        .mark_profile_dirty(&profile_id, Some("manifest-fingerprint-v2"))
        .expect("mark profile dirty");
    let dirty_profile = repository
        .profile_state(&profile_id)
        .expect("read dirty profile state")
        .expect("dirty profile state exists");
    assert_eq!(
        dirty_profile.readiness,
        ModLibraryProjectionReadiness::Dirty
    );
    assert!(!dirty_profile.is_complete_for("manifest-fingerprint-v2"));
}

#[test]
fn projection_rebuild_removes_stale_rows_and_advances_generation() {
    let temp = tempfile::tempdir().expect("temporary app data");
    let conn = crate::open_database(&temp.path().join("hmm.db")).expect("open database");
    let conn = Arc::new(Mutex::new(conn));
    let repository = SqliteModLibraryProjectionRepository::new(Arc::clone(&conn));
    let profile_id = ProfileId::new("stale-profile");

    repository
        .rebuild(&ModLibraryProjectionSnapshot {
            source_fingerprint: "catalog-v1".to_owned(),
            records: vec![record("stale-mod", "Stale")],
            profiles: vec![ModLibraryProfileProjection {
                profile_id: profile_id.clone(),
                source_fingerprint: "manifest-v1".to_owned(),
                statuses: vec![],
            }],
        })
        .expect("publish first projection");
    let state = repository
        .rebuild(&ModLibraryProjectionSnapshot {
            source_fingerprint: "catalog-v2".to_owned(),
            records: vec![record("current-mod", "Current")],
            profiles: vec![],
        })
        .expect("publish replacement projection");

    assert_eq!(state.generation, 2);
    assert!(state.is_complete_for("catalog-v2"));
    assert!(repository
        .profile_state(&profile_id)
        .expect("read removed profile")
        .is_none());
    let conn = conn.lock().expect("database lock");
    let stale_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM mod_library_projection_items WHERE mod_id = 'stale-mod'",
            [],
            |row| row.get(0),
        )
        .expect("count stale projection rows");
    let current_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM mod_library_projection_items WHERE mod_id = 'current-mod'",
            [],
            |row| row.get(0),
        )
        .expect("count current projection rows");
    assert_eq!(stale_count, 0);
    assert_eq!(current_count, 1);
}

#[test]
fn replace_profile_publishes_a_complete_incremented_generation() {
    let temp = tempfile::tempdir().expect("temporary app data");
    let conn = crate::open_database(&temp.path().join("hmm.db")).expect("open database");
    let conn = Arc::new(Mutex::new(conn));
    let repository = SqliteModLibraryProjectionRepository::new(Arc::clone(&conn));
    repository
        .rebuild(&ModLibraryProjectionSnapshot {
            source_fingerprint: "catalog".to_owned(),
            records: vec![record("mod-a", "Mod A")],
            profiles: vec![],
        })
        .expect("publish global projection");
    let profile_id = ProfileId::new("profile-a");

    let state = repository
        .replace_profile(&ModLibraryProfileProjection {
            profile_id: profile_id.clone(),
            source_fingerprint: "manifest-v1".to_owned(),
            statuses: vec![ModLibraryProjectionStatusRecord {
                mod_id: ModId::new("mod-a"),
                status: ModLibraryProjectionStatus::CleanupPending,
                managed_file_count: 3,
                backup_count: 1,
            }],
        })
        .expect("publish profile projection");

    assert_eq!(state.generation, 1);
    assert!(state.is_complete_for("manifest-v1"));
    let conn = conn.lock().expect("database lock");
    let row: (i64, String) = conn
        .query_row(
            "SELECT profile_generation, status
             FROM mod_library_projection_profile_status
             WHERE profile_id = 'profile-a' AND mod_id = 'mod-a'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read profile status row");
    assert_eq!(row, (1, "cleanup_pending".to_owned()));
}

#[test]
fn replace_profile_with_unknown_mod_fails_closed_and_keeps_dirty_state() {
    let temp = tempfile::tempdir().expect("temporary app data");
    let conn = crate::open_database(&temp.path().join("hmm.db")).expect("open database");
    let conn = Arc::new(Mutex::new(conn));
    let repository = SqliteModLibraryProjectionRepository::new(Arc::clone(&conn));
    repository
        .rebuild(&ModLibraryProjectionSnapshot {
            source_fingerprint: "catalog".to_owned(),
            records: vec![record("known-mod", "Known")],
            profiles: vec![],
        })
        .expect("publish global projection");
    let profile_id = ProfileId::new("profile-a");

    let error = repository
        .replace_profile(&ModLibraryProfileProjection {
            profile_id: profile_id.clone(),
            source_fingerprint: "manifest".to_owned(),
            statuses: vec![ModLibraryProjectionStatusRecord {
                mod_id: ModId::new("unknown-mod"),
                status: ModLibraryProjectionStatus::Installed,
                managed_file_count: 1,
                backup_count: 0,
            }],
        })
        .expect_err("unknown Mod status must not publish");
    assert!(error
        .to_string()
        .contains("failed to insert Mod library profile status"));
    let state = repository
        .profile_state(&profile_id)
        .expect("read profile state")
        .expect("dirty profile state exists");
    assert_eq!(state.generation, 0);
    assert_eq!(state.readiness, ModLibraryProjectionReadiness::Dirty);
    assert_eq!(state.source_fingerprint.as_deref(), Some("manifest"));
    let conn = conn.lock().expect("database lock");
    let status_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM mod_library_projection_profile_status WHERE profile_id = 'profile-a'",
            [],
            |row| row.get(0),
        )
        .expect("count failed profile status rows");
    assert_eq!(status_count, 0);
}

#[test]
fn failed_rebuild_keeps_projection_fail_closed_and_does_not_publish_partial_rows() {
    let temp = tempfile::tempdir().expect("temporary app data");
    let conn = crate::open_database(&temp.path().join("hmm.db")).expect("open database");
    let repository = SqliteModLibraryProjectionRepository::new(Arc::new(Mutex::new(conn)));
    repository
        .rebuild(&ModLibraryProjectionSnapshot {
            source_fingerprint: "good".to_owned(),
            records: vec![record("mod-a", "Stable")],
            profiles: vec![],
        })
        .expect("publish baseline projection");

    let error = repository
        .rebuild(&ModLibraryProjectionSnapshot {
            source_fingerprint: "bad".to_owned(),
            records: vec![record("mod-a", "Duplicate"), record("mod-a", "Duplicate")],
            profiles: vec![],
        })
        .expect_err("duplicate Mod ids must fail before publish");
    assert!(error.to_string().contains("duplicate Mod ids"));

    let state = repository.state().expect("read failed projection state");
    assert_eq!(state.readiness, ModLibraryProjectionReadiness::Dirty);
    assert_eq!(state.generation, 1);
    assert_eq!(state.source_fingerprint.as_deref(), Some("bad"));
    assert!(!state.is_complete_for("good"));
}

#[test]
fn projection_query_keeps_totals_clamp_sort_filters_and_unicode_in_one_snapshot() {
    let temp = tempfile::tempdir().expect("temporary app data");
    let conn = crate::open_database(&temp.path().join("hmm.db")).expect("open database");
    conn.execute(
        "INSERT INTO categories (category_id, name, color, sort_order, created_at)
         VALUES ('category-a', 'Quest', NULL, 0, 1), ('category-b', 'Visual', NULL, 1, 1)",
        [],
    )
    .expect("seed categories");
    let conn = Arc::new(Mutex::new(conn));
    let repository = SqliteModLibraryProjectionRepository::new(Arc::clone(&conn));
    let mut beta = record("mod-b", "Beta");
    beta.labels[0].category_id = Some("category-b".to_owned());
    beta.labels[0].name = "Visual".to_owned();
    let mut cafe = record("mod-c", "Cafe\u{301}");
    cafe.labels[0].name = "Other".to_owned();
    repository
        .rebuild(&ModLibraryProjectionSnapshot {
            source_fingerprint: "catalog-v1".to_owned(),
            records: vec![record("mod-a", "  Ａrmor\u{3000}CAFÉ  "), beta, cafe],
            profiles: vec![ModLibraryProfileProjection {
                profile_id: ProfileId::new("profile-a"),
                source_fingerprint: "manifest-v1".to_owned(),
                statuses: vec![
                    ModLibraryProjectionStatusRecord {
                        mod_id: ModId::new("mod-b"),
                        status: ModLibraryProjectionStatus::Installed,
                        managed_file_count: 2,
                        backup_count: 1,
                    },
                    ModLibraryProjectionStatusRecord {
                        mod_id: ModId::new("mod-c"),
                        status: ModLibraryProjectionStatus::CleanupPending,
                        managed_file_count: 3,
                        backup_count: 0,
                    },
                ],
            }],
        })
        .expect("publish projection");

    let page = repository
        .query(&ModLibraryProjectionQueryRequest {
            source_fingerprint: "catalog-v1".to_owned(),
            profile: Some(ModLibraryProjectionProfileQuery {
                profile_id: ProfileId::new("profile-a"),
                source_fingerprint: "manifest-v1".to_owned(),
            }),
            normalized_search: "café".to_owned(),
            filter: ModLibraryProjectionQueryFilter::All,
            page: u64::MAX,
            page_size: 12,
        })
        .expect("query projection");
    assert_eq!(page.library_total, 3);
    assert_eq!(page.matching_total, 2);
    assert_eq!(page.page, 1);
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].record.mod_id.as_str(), "mod-a");
    assert!(page.items[0].status.is_none());
    assert_eq!(page.items[1].record.mod_id.as_str(), "mod-c");
    assert_eq!(
        page.items[1].status.as_ref().map(|status| status.status),
        Some(ModLibraryProjectionStatus::CleanupPending)
    );

    let installed = repository
        .query(&ModLibraryProjectionQueryRequest {
            source_fingerprint: "catalog-v1".to_owned(),
            profile: Some(ModLibraryProjectionProfileQuery {
                profile_id: ProfileId::new("profile-a"),
                source_fingerprint: "manifest-v1".to_owned(),
            }),
            normalized_search: String::new(),
            filter: ModLibraryProjectionQueryFilter::Status(
                ModLibraryProjectionQueryStatus::Installed,
            ),
            page: 1,
            page_size: 12,
        })
        .expect("status filter");
    assert_eq!(installed.matching_total, 1);
    assert_eq!(installed.items[0].record.mod_id.as_str(), "mod-b");

    let category = repository
        .query(&ModLibraryProjectionQueryRequest {
            source_fingerprint: "catalog-v1".to_owned(),
            profile: None,
            normalized_search: String::new(),
            filter: ModLibraryProjectionQueryFilter::Category("category-b".to_owned()),
            page: 1,
            page_size: 12,
        })
        .expect("category filter");
    assert_eq!(category.matching_total, 1);
    assert_eq!(category.items[0].record.mod_id.as_str(), "mod-b");
}

#[test]
fn projection_query_fails_closed_for_dirty_or_mismatched_generations() {
    let temp = tempfile::tempdir().expect("temporary app data");
    let conn = crate::open_database(&temp.path().join("hmm.db")).expect("open database");
    let conn = Arc::new(Mutex::new(conn));
    let repository = SqliteModLibraryProjectionRepository::new(Arc::clone(&conn));
    repository
        .rebuild(&ModLibraryProjectionSnapshot {
            source_fingerprint: "catalog-v1".to_owned(),
            records: vec![record("mod-a", "Alpha")],
            profiles: vec![ModLibraryProfileProjection {
                profile_id: ProfileId::new("profile-a"),
                source_fingerprint: "manifest-v1".to_owned(),
                statuses: vec![],
            }],
        })
        .expect("publish projection");
    let request = ModLibraryProjectionQueryRequest {
        source_fingerprint: "catalog-v1".to_owned(),
        profile: Some(ModLibraryProjectionProfileQuery {
            profile_id: ProfileId::new("profile-a"),
            source_fingerprint: "manifest-v1".to_owned(),
        }),
        normalized_search: String::new(),
        filter: ModLibraryProjectionQueryFilter::All,
        page: 1,
        page_size: 12,
    };
    repository
        .query(&request)
        .expect("complete projection is readable");

    let mut wrong_source = request.clone();
    wrong_source.source_fingerprint = "catalog-v2".to_owned();
    assert_eq!(
        repository.query(&wrong_source),
        Err(ModLibraryProjectionQueryError::Unavailable)
    );

    repository
        .mark_dirty(Some("catalog-v2"))
        .expect("mark projection dirty");
    assert_eq!(
        repository.query(&request),
        Err(ModLibraryProjectionQueryError::Unavailable)
    );

    repository
        .rebuild(&ModLibraryProjectionSnapshot {
            source_fingerprint: "catalog-v2".to_owned(),
            records: vec![record("mod-a", "Alpha")],
            profiles: vec![],
        })
        .expect("publish second projection");
    let mut missing_profile = request;
    missing_profile.source_fingerprint = "catalog-v2".to_owned();
    assert_eq!(
        repository.query(&missing_profile),
        Err(ModLibraryProjectionQueryError::ProfileUnavailable)
    );
}

fn record(mod_id: &str, display_name: &str) -> ModLibraryProjectionRecord {
    ModLibraryProjectionRecord {
        mod_id: ModId::new(mod_id),
        display_revision_id: ModRevisionId::new(format!("revision-{mod_id}")),
        package_id: format!("package-{mod_id}"),
        display_name: display_name.to_owned(),
        author: Some("Fixture Author".to_owned()),
        version_label: Some("v1.0".to_owned()),
        size_label: "Imported".to_owned(),
        preview_image: StoredImportPreviewImage::Fallback {
            reason: hmm_core::PreviewImageRejectionReason::Missing,
        },
        labels: vec![ModLibraryProjectionLabel {
            category_id: Some("category-a".to_owned()),
            name: "Armor".to_owned(),
            color: Some("#123456".to_owned()),
        }],
    }
}
