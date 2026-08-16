use rusqlite_migration::{Migrations, M};

pub(crate) fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("migrations/001_metadata_categories.sql")),
        M::up(include_str!("migrations/002_profiles.sql")),
        M::up(include_str!("migrations/003_profile_save_settings.sql")),
        M::up(include_str!("migrations/004_save_backups.sql")),
        M::up(include_str!(
            "migrations/005_save_backup_directory_snapshot.sql"
        )),
        M::up(include_str!(
            "migrations/006_save_backup_scheduler_state.sql"
        )),
        M::up(include_str!(
            "migrations/007_save_backup_worker_heartbeat.sql"
        )),
        M::up(include_str!(
            "migrations/008_save_backup_background_settings.sql"
        )),
        M::up(include_str!("migrations/009_mod_library_projection.sql")),
        M::up(include_str!("migrations/010_external_import_preview.sql")),
        M::up(include_str!("migrations/011_batch_lifecycle.sql")),
        M::up(include_str!("migrations/012_save_restore.sql")),
        M::up(include_str!(
            "migrations/013_save_backup_retention_center.sql"
        )),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_migration_adds_nullable_worker_heartbeat() {
        let mut conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        let migrations = migrations();
        migrations
            .to_version(&mut conn, 6)
            .expect("migrate through 006");
        conn.execute(
            "INSERT INTO save_backup_scheduler_state (
                game_id, profile_id, enabled, background_protection_enabled,
                background_status, last_checked_at, worker_instance_id, updated_at
             ) VALUES (?1, ?2, 1, 1, 'tray_only', ?3, ?4, ?3)",
            rusqlite::params!["mhw", "legacy-profile", 1_234_i64, "legacy-worker"],
        )
        .expect("insert legacy scheduler row");

        migrations.to_latest(&mut conn).expect("migrate to latest");
        let heartbeat: Option<i64> = conn
            .query_row(
                "SELECT worker_heartbeat_at
                 FROM save_backup_scheduler_state
                 WHERE game_id = 'mhw' AND profile_id = 'legacy-profile'",
                [],
                |row| row.get(0),
            )
            .expect("read migrated heartbeat");
        assert_eq!(
            heartbeat, None,
            "migration must not forge heartbeat from last_checked_at"
        );
    }

    #[test]
    fn background_settings_migration_preserves_scheduler_state_without_forging_singleton() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut conn = rusqlite::Connection::open(temp.path().join("migration.db"))
            .expect("open temporary database");
        let migrations = migrations();
        migrations
            .to_version(&mut conn, 7)
            .expect("migrate through 007");
        conn.execute(
            "INSERT INTO save_backup_scheduler_state (
                game_id, profile_id, enabled, background_protection_enabled,
                background_status, worker_heartbeat_at, updated_at
             ) VALUES (?1, ?2, 1, 1, 'protected', ?3, ?4)",
            rusqlite::params!["mhw", "legacy-profile", 1_234_i64, 1_235_i64],
        )
        .expect("insert scheduler row before background settings migration");

        migrations
            .to_latest(&mut conn)
            .expect("migrate through 008");

        let scheduler_state: (String, Option<i64>, i64) = conn
            .query_row(
                "SELECT background_status, worker_heartbeat_at, updated_at
                 FROM save_backup_scheduler_state
                 WHERE game_id = 'mhw' AND profile_id = 'legacy-profile'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read preserved scheduler state");
        assert_eq!(
            scheduler_state,
            ("protected".to_owned(), Some(1_234), 1_235)
        );

        let singleton_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM save_backup_background_settings",
                [],
                |row| row.get(0),
            )
            .expect("count background settings rows");
        assert_eq!(
            singleton_count, 0,
            "migration must not invent a background protection intent"
        );
    }

    #[test]
    fn save_restore_migration_defaults_existing_profiles_to_pre_restore_backup_enabled() {
        let mut conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        let migrations = migrations();
        migrations
            .to_version(&mut conn, 11)
            .expect("migrate through 011");
        conn.execute(
            "INSERT INTO profile_save_settings (
                profile_id, backup_cadence, backup_weekdays,
                retention_max_count, updated_at
             ) VALUES ('default', 'manual', '[]', 20, 42)",
            [],
        )
        .expect("insert legacy save settings");

        migrations
            .to_latest(&mut conn)
            .expect("migrate through 012");

        let enabled: i64 = conn
            .query_row(
                "SELECT pre_restore_backup_enabled
                 FROM profile_save_settings WHERE profile_id = 'default'",
                [],
                |row| row.get(0),
            )
            .expect("read migrated restore setting");
        assert_eq!(enabled, 1);
    }

    #[test]
    fn retention_center_migration_keeps_space_budget_and_account_snapshot_disabled() {
        let mut conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        let migrations = migrations();
        migrations
            .to_version(&mut conn, 12)
            .expect("migrate through 012");
        conn.execute(
            "INSERT INTO profile_save_settings (
                profile_id, backup_cadence, backup_weekdays,
                retention_max_count, updated_at
             ) VALUES ('default', 'manual', '[]', 20, 42)",
            [],
        )
        .expect("insert legacy save settings");

        migrations
            .to_latest(&mut conn)
            .expect("migrate through 013");

        let values: (Option<i64>, Option<String>) = conn
            .query_row(
                "SELECT retention_max_total_bytes, steam_account_label
                 FROM profile_save_settings WHERE profile_id = 'default'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read migrated retention settings");
        assert_eq!(values, (None, None));
        assert!(conn
            .execute(
                "UPDATE profile_save_settings
                 SET retention_max_total_bytes = -1
                 WHERE profile_id = 'default'",
                [],
            )
            .is_err());
    }
}
