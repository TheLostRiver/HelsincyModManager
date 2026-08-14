mod batch_lifecycle_repository;
mod category_repository;
mod external_import_batch_repository;
mod migrations;
mod mod_library_projection_repository;
mod mod_metadata_repository;
mod profile_repository;
mod save_backup_background_settings_repository;
mod save_backup_repository;
mod save_backup_scheduler_repository;
mod save_restore_transaction_repository;

pub use batch_lifecycle_repository::SqliteBatchLifecycleRepository;
pub use category_repository::SqliteCategoryRepository;
pub use external_import_batch_repository::SqliteExternalImportBatchRepository;
pub use mod_library_projection_repository::SqliteModLibraryProjectionRepository;
pub use mod_metadata_repository::SqliteModMetadataRepository;
pub use profile_repository::SqliteProfileRepository;
pub use save_backup_background_settings_repository::SqliteSaveBackupBackgroundSettingsRepository;
pub use save_backup_repository::SqliteSaveBackupRepository;
pub use save_backup_scheduler_repository::SqliteSaveBackupSchedulerStateRepository;
pub use save_restore_transaction_repository::SqliteSaveRestoreTransactionRepository;

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
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

pub fn open_database_read_only(path: &Path) -> Result<Connection> {
    let wal_path = sqlite_sidecar_path(path, "-wal");
    let shm_path = sqlite_sidecar_path(path, "-shm");
    let wal_exists = sqlite_sidecar_entry_exists(&wal_path)?;
    let shm_exists = sqlite_sidecar_entry_exists(&shm_path)?;
    if wal_exists || shm_exists {
        anyhow::bail!("read-only database has active WAL sidecar state");
    }
    let conn = Connection::open_with_flags(
        sqlite_immutable_uri(path),
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("failed to open database read-only: {}", path.display()))?;

    conn.pragma_update(None, "query_only", "ON")
        .context("failed to enable SQLite query-only mode")?;
    conn.busy_timeout(Duration::from_secs(5))
        .context("failed to set read-only database busy timeout")?;

    Ok(conn)
}

fn sqlite_immutable_uri(path: &Path) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    let normalized = path.as_os_str().to_string_lossy().replace('\\', "/");
    let mut uri = String::with_capacity(normalized.len() + "file:?immutable=1".len());
    uri.push_str("file:");
    for byte in normalized.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b':' | b'-' | b'.' | b'_' | b'~') {
            uri.push(char::from(byte));
        } else {
            uri.push('%');
            uri.push(char::from(HEX[usize::from(byte >> 4)]));
            uri.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    uri.push_str("?immutable=1");
    uri
}

fn sqlite_sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut sidecar = path.as_os_str().to_os_string();
    sidecar.push(suffix);
    sidecar.into()
}

fn sqlite_sidecar_entry_exists(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error)
            .with_context(|| format!("failed to inspect SQLite sidecar: {}", path.display())),
    }
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

    #[test]
    fn read_only_open_does_not_create_missing_database_or_parent() {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("missing").join("state.db");

        assert!(open_database_read_only(&db_path).is_err());
        assert!(!db_path.exists());
        assert!(!db_path.parent().expect("database parent").exists());
    }

    #[test]
    fn read_only_open_rejects_mutation_without_changing_existing_database() {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("state.db");
        let conn = Connection::open(&db_path).expect("create fixture database");
        conn.execute("CREATE TABLE fixture (value TEXT NOT NULL)", [])
            .expect("create fixture table");
        conn.execute("INSERT INTO fixture (value) VALUES ('before')", [])
            .expect("insert fixture row");
        drop(conn);
        let before = std::fs::read(&db_path).expect("read fixture database");

        let conn = open_database_read_only(&db_path).expect("open fixture read-only");
        let value: String = conn
            .query_row("SELECT value FROM fixture", [], |row| row.get(0))
            .expect("read fixture row");
        assert_eq!(value, "before");
        assert!(conn
            .execute("INSERT INTO fixture (value) VALUES ('after')", [])
            .is_err());
        drop(conn);

        assert_eq!(
            std::fs::read(&db_path).expect("read unchanged fixture database"),
            before
        );
        assert!(!sqlite_sidecar_path(&db_path, "-wal").exists());
        assert!(!sqlite_sidecar_path(&db_path, "-shm").exists());
    }

    #[test]
    fn read_only_open_rejects_non_file_sidecar_without_changing_state() {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("state.db");
        let conn = Connection::open(&db_path).expect("create fixture database");
        conn.execute("CREATE TABLE fixture (value TEXT NOT NULL)", [])
            .expect("create fixture table");
        drop(conn);
        let database_before = std::fs::read(&db_path).expect("read fixture database");
        let wal_path = sqlite_sidecar_path(&db_path, "-wal");
        let sentinel_path = wal_path.join("sentinel");
        std::fs::create_dir(&wal_path).expect("create non-file WAL sidecar");
        std::fs::write(&sentinel_path, b"unchanged").expect("write sidecar sentinel");

        assert!(open_database_read_only(&db_path).is_err());

        assert_eq!(
            std::fs::read(&db_path).expect("read unchanged fixture database"),
            database_before
        );
        assert_eq!(
            std::fs::read(&sentinel_path).expect("read unchanged sidecar sentinel"),
            b"unchanged"
        );
        assert!(!sqlite_sidecar_path(&db_path, "-shm").exists());
    }

    #[test]
    fn read_only_open_reads_checkpointed_wal_mode_database_without_creating_sidecars() {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("state.db");
        let conn = Connection::open(&db_path).expect("create WAL database");
        let journal_mode: String = conn
            .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
            .expect("enable WAL mode");
        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
        conn.execute("CREATE TABLE fixture (value TEXT NOT NULL)", [])
            .expect("create fixture table");
        conn.execute("INSERT INTO fixture (value) VALUES ('before')", [])
            .expect("insert fixture row");
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("checkpoint WAL fixture");
        drop(conn);
        let wal_path = sqlite_sidecar_path(&db_path, "-wal");
        let shm_path = sqlite_sidecar_path(&db_path, "-shm");
        if wal_path.exists() {
            std::fs::remove_file(&wal_path).expect("remove checkpointed WAL sidecar");
        }
        if shm_path.exists() {
            std::fs::remove_file(&shm_path).expect("remove checkpointed SHM sidecar");
        }
        let database_before = std::fs::read(&db_path).expect("read WAL database before");
        assert!(!wal_path.exists());
        assert!(!shm_path.exists());

        let conn = open_database_read_only(&db_path).expect("open clean WAL database read-only");
        let value: String = conn
            .query_row("SELECT value FROM fixture", [], |row| row.get(0))
            .expect("read fixture row");

        assert_eq!(value, "before");
        drop(conn);
        assert_eq!(
            std::fs::read(&db_path).expect("read unchanged WAL database"),
            database_before
        );
        assert!(!wal_path.exists());
        assert!(!shm_path.exists());
    }

    #[test]
    fn read_only_open_does_not_create_shm_for_orphaned_wal_snapshot() {
        let temp = tempfile::tempdir().expect("temp dir");
        let live_db_path = temp.path().join("live.db");
        let live_conn = Connection::open(&live_db_path).expect("create live WAL database");
        let journal_mode: String = live_conn
            .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
            .expect("enable WAL mode");
        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
        live_conn
            .pragma_update(None, "wal_autocheckpoint", 0)
            .expect("disable automatic checkpoint");
        live_conn
            .execute_batch(
                "CREATE TABLE fixture (value TEXT NOT NULL);
                 INSERT INTO fixture (value) VALUES ('from-wal');",
            )
            .expect("write fixture into WAL");

        let snapshot_dir = temp.path().join("snapshot");
        std::fs::create_dir(&snapshot_dir).expect("create snapshot directory");
        let snapshot_db_path = snapshot_dir.join("state.db");
        let live_wal_path = sqlite_sidecar_path(&live_db_path, "-wal");
        let snapshot_wal_path = sqlite_sidecar_path(&snapshot_db_path, "-wal");
        let snapshot_shm_path = sqlite_sidecar_path(&snapshot_db_path, "-shm");
        std::fs::copy(&live_db_path, &snapshot_db_path).expect("copy main database");
        std::fs::copy(&live_wal_path, &snapshot_wal_path).expect("copy WAL");
        let database_before =
            std::fs::read(&snapshot_db_path).expect("read snapshot database before");
        let wal_before = std::fs::read(&snapshot_wal_path).expect("read snapshot WAL before");
        assert!(!snapshot_shm_path.exists());

        let opened = open_database_read_only(&snapshot_db_path);

        assert!(opened.is_err(), "incomplete WAL state must fail closed");
        assert!(
            !snapshot_shm_path.exists(),
            "read-only open must not create a shared-memory sidecar"
        );
        drop(opened);
        assert_eq!(
            std::fs::read(&snapshot_db_path).expect("read unchanged snapshot database"),
            database_before
        );
        assert_eq!(
            std::fs::read(&snapshot_wal_path).expect("read unchanged snapshot WAL"),
            wal_before
        );
    }

    #[test]
    fn read_only_open_rejects_live_wal_without_changing_sidecars() {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("state.db");
        let writer = Connection::open(&db_path).expect("create live WAL database");
        let journal_mode: String = writer
            .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
            .expect("enable WAL mode");
        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
        writer
            .pragma_update(None, "wal_autocheckpoint", 0)
            .expect("disable automatic checkpoint");
        writer
            .execute_batch(
                "CREATE TABLE fixture (value TEXT NOT NULL);
                 INSERT INTO fixture (value) VALUES ('from-live-wal');",
            )
            .expect("write live WAL fixture");
        let wal_path = sqlite_sidecar_path(&db_path, "-wal");
        let shm_path = sqlite_sidecar_path(&db_path, "-shm");
        assert!(wal_path.exists());
        assert!(shm_path.exists());
        let database_before = std::fs::read(&db_path).expect("read live database before");
        let wal_before = std::fs::read(&wal_path).expect("read live WAL before");
        let shm_before = std::fs::read(&shm_path).expect("read live SHM before");

        let opened = open_database_read_only(&db_path);

        assert!(opened.is_err(), "live WAL state must fail closed");
        assert_eq!(
            std::fs::read(&db_path).expect("read unchanged live database"),
            database_before
        );
        assert_eq!(
            std::fs::read(&wal_path).expect("read unchanged live WAL"),
            wal_before
        );
        assert_eq!(
            std::fs::read(&shm_path).expect("read unchanged live SHM"),
            shm_before
        );
    }
}
