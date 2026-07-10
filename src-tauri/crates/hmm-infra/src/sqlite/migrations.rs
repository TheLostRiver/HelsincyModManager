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

        migrations
            .to_latest(&mut conn)
            .expect("migrate through 007");
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
}
