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
}
