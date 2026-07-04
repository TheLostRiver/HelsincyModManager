mod category_repository;
mod migrations;
mod mod_metadata_repository;
mod profile_repository;
mod save_backup_repository;

pub use category_repository::SqliteCategoryRepository;
pub use mod_metadata_repository::SqliteModMetadataRepository;
pub use profile_repository::SqliteProfileRepository;
pub use save_backup_repository::SqliteSaveBackupRepository;

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;
use std::time::Duration;

pub fn open_database(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("failed to create database directory: {}", parent.display())
        })?;
    }

    let mut conn = Connection::open(path)
        .with_context(|| format!("failed to open database: {}", path.display()))?;

    conn.pragma_update(None, "foreign_keys", "ON")
        .context("failed to enable foreign keys")?;

    let journal_mode: String = conn
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .context("failed to set WAL journal mode")?;

    if journal_mode.to_lowercase() != "wal" {
        anyhow::bail!("expected WAL journal mode but got '{journal_mode}'");
    }

    conn.busy_timeout(Duration::from_secs(5))
        .context("failed to set busy timeout")?;

    migrations::migrations()
        .to_latest(&mut conn)
        .context("failed to run database migrations")?;

    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_database_creates_file_and_schema() {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("test.db");

        let conn = open_database(&db_path).unwrap();

        assert!(db_path.exists());

        // Verify mod_metadata table exists
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='mod_metadata'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Verify categories table exists
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='categories'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Verify mod_categories table exists
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='mod_categories'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Verify profiles table exists
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='profiles'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migration_is_idempotent() {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("test.db");

        let conn1 = open_database(&db_path).unwrap();
        drop(conn1);

        // Second open should not panic or fail
        let _conn2 = open_database(&db_path).unwrap();
    }

    #[test]
    fn foreign_keys_enabled() {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("test.db");
        let conn = open_database(&db_path).unwrap();

        let fk: i32 = conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn wal_mode_enabled() {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("test.db");
        let conn = open_database(&db_path).unwrap();

        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn foreign_key_cascade_works() {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("test.db");
        let conn = open_database(&db_path).unwrap();

        // Insert a category
        conn.execute(
            "INSERT INTO categories (category_id, name, sort_order, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["cat-1", "Test Category", 0, 1000],
        )
        .unwrap();

        // Assign a mod to the category
        conn.execute(
            "INSERT INTO mod_categories (mod_id, category_id) VALUES (?1, ?2)",
            rusqlite::params!["mod-1", "cat-1"],
        )
        .unwrap();

        // Delete the category — should cascade to mod_categories
        conn.execute(
            "DELETE FROM categories WHERE category_id = ?1",
            rusqlite::params!["cat-1"],
        )
        .unwrap();

        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM mod_categories WHERE category_id = ?1",
                rusqlite::params!["cat-1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "cascade delete should remove mod_categories rows");
    }

    #[test]
    fn migration_creates_default_active_profile() {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("test.db");
        let conn = open_database(&db_path).unwrap();

        let (profile_id, name, is_active, created_at, updated_at): (String, String, i32, i64, i64) =
            conn.query_row(
                "SELECT profile_id, name, is_active, created_at, updated_at
                 FROM profiles WHERE profile_id = 'default'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(profile_id, "default");
        assert_eq!(name, "Default");
        assert_eq!(is_active, 1);
        assert!(created_at > 0);
        assert_eq!(updated_at, created_at);
    }

    #[test]
    fn creates_parent_directories() {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("nested").join("dirs").join("test.db");

        let _conn = open_database(&db_path).unwrap();
        assert!(db_path.exists());
    }
}
