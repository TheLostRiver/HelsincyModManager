use anyhow::{anyhow, Context, Result};
use hmm_core::{ModId, ModMetadataOverlay};
use hmm_ports::ModMetadataRepository;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub struct SqliteModMetadataRepository {
    db: Arc<Mutex<Connection>>,
}

impl SqliteModMetadataRepository {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| anyhow!("database lock poisoned"))
    }
}

impl ModMetadataRepository for SqliteModMetadataRepository {
    fn get(&self, mod_id: &str) -> Result<Option<ModMetadataOverlay>> {
        let conn = self.lock_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT mod_id, display_name, author, version, description, nexus_mod_id, updated_at
                 FROM mod_metadata WHERE mod_id = ?1",
            )
            .context("failed to prepare get metadata query")?;

        let result = stmt.query_row(rusqlite::params![mod_id], |row| {
            Ok(ModMetadataOverlay {
                mod_id: ModId::new(row.get::<_, String>(0)?),
                display_name: row.get(1)?,
                author: row.get(2)?,
                version: row.get(3)?,
                description: row.get(4)?,
                nexus_mod_id: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                updated_at: row.get::<_, i64>(6)? as u128,
            })
        });

        match result {
            Ok(overlay) => Ok(Some(overlay)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error).context("failed to get mod metadata"),
        }
    }

    fn save(&self, overlay: &ModMetadataOverlay) -> Result<()> {
        let conn = self.lock_db()?;
        conn.execute(
            "INSERT OR REPLACE INTO mod_metadata
                (mod_id, display_name, author, version, description, nexus_mod_id, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                overlay.mod_id.as_str(),
                overlay.display_name,
                overlay.author,
                overlay.version,
                overlay.description,
                overlay.nexus_mod_id.map(|v| v as i64),
                overlay.updated_at as i64,
            ],
        )
        .context("failed to save mod metadata")?;
        Ok(())
    }

    fn delete(&self, mod_id: &str) -> Result<()> {
        let conn = self.lock_db()?;
        conn.execute(
            "DELETE FROM mod_metadata WHERE mod_id = ?1",
            rusqlite::params![mod_id],
        )
        .context("failed to delete mod metadata")?;
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<ModMetadataOverlay>> {
        let conn = self.lock_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT mod_id, display_name, author, version, description, nexus_mod_id, updated_at
                 FROM mod_metadata",
            )
            .context("failed to prepare list metadata query")?;

        let rows = stmt
            .query_map([], |row| {
                Ok(ModMetadataOverlay {
                    mod_id: ModId::new(row.get::<_, String>(0)?),
                    display_name: row.get(1)?,
                    author: row.get(2)?,
                    version: row.get(3)?,
                    description: row.get(4)?,
                    nexus_mod_id: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                    updated_at: row.get::<_, i64>(6)? as u128,
                })
            })
            .context("failed to list mod metadata")?;

        let mut overlays = Vec::new();
        for row in rows {
            overlays.push(row.context("failed to read mod metadata row")?);
        }
        Ok(overlays)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::open_database;

    fn test_repo() -> SqliteModMetadataRepository {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("test.db");
        let conn = open_database(&db_path).unwrap();
        // Keep tempdir alive by leaking it (tests are short-lived)
        std::mem::forget(temp);
        SqliteModMetadataRepository::new(Arc::new(Mutex::new(conn)))
    }

    fn sample_overlay(mod_id: &str) -> ModMetadataOverlay {
        ModMetadataOverlay {
            mod_id: ModId::new(mod_id),
            display_name: Some("Custom Name".to_owned()),
            author: Some("Test Author".to_owned()),
            version: Some("2.0.0".to_owned()),
            description: Some("A test mod".to_owned()),
            nexus_mod_id: Some(12345),
            updated_at: 1000,
        }
    }

    #[test]
    fn get_returns_none_for_missing_mod() {
        let repo = test_repo();
        let result = repo.get("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn save_and_get_round_trips() {
        let repo = test_repo();
        let overlay = sample_overlay("mod-1");

        repo.save(&overlay).unwrap();
        let loaded = repo.get("mod-1").unwrap().expect("should exist");

        assert_eq!(loaded.mod_id.as_str(), "mod-1");
        assert_eq!(loaded.display_name, Some("Custom Name".to_owned()));
        assert_eq!(loaded.author, Some("Test Author".to_owned()));
        assert_eq!(loaded.version, Some("2.0.0".to_owned()));
        assert_eq!(loaded.description, Some("A test mod".to_owned()));
        assert_eq!(loaded.nexus_mod_id, Some(12345));
        assert_eq!(loaded.updated_at, 1000);
    }

    #[test]
    fn save_with_none_fields_stores_nulls() {
        let repo = test_repo();
        let overlay = ModMetadataOverlay {
            mod_id: ModId::new("mod-sparse"),
            display_name: Some("Only Name".to_owned()),
            author: None,
            version: None,
            description: None,
            nexus_mod_id: None,
            updated_at: 2000,
        };

        repo.save(&overlay).unwrap();
        let loaded = repo.get("mod-sparse").unwrap().expect("should exist");

        assert_eq!(loaded.display_name, Some("Only Name".to_owned()));
        assert!(loaded.author.is_none());
        assert!(loaded.version.is_none());
        assert!(loaded.description.is_none());
        assert!(loaded.nexus_mod_id.is_none());
    }

    #[test]
    fn save_replaces_existing_overlay() {
        let repo = test_repo();
        let mut overlay = sample_overlay("mod-1");
        repo.save(&overlay).unwrap();

        overlay.display_name = Some("Updated Name".to_owned());
        overlay.updated_at = 3000;
        repo.save(&overlay).unwrap();

        let loaded = repo.get("mod-1").unwrap().expect("should exist");
        assert_eq!(loaded.display_name, Some("Updated Name".to_owned()));
        assert_eq!(loaded.updated_at, 3000);
    }

    #[test]
    fn delete_removes_overlay() {
        let repo = test_repo();
        repo.save(&sample_overlay("mod-1")).unwrap();

        repo.delete("mod-1").unwrap();

        let result = repo.get("mod-1").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn delete_nonexistent_is_no_op() {
        let repo = test_repo();
        repo.delete("nonexistent").unwrap();
    }

    #[test]
    fn list_all_returns_all_overlays() {
        let repo = test_repo();
        repo.save(&sample_overlay("mod-a")).unwrap();
        repo.save(&sample_overlay("mod-b")).unwrap();
        repo.save(&sample_overlay("mod-c")).unwrap();

        let all = repo.list_all().unwrap();
        assert_eq!(all.len(), 3);

        let ids: Vec<&str> = all.iter().map(|o| o.mod_id.as_str()).collect();
        assert!(ids.contains(&"mod-a"));
        assert!(ids.contains(&"mod-b"));
        assert!(ids.contains(&"mod-c"));
    }

    #[test]
    fn list_all_returns_empty_when_no_overlays() {
        let repo = test_repo();
        let all = repo.list_all().unwrap();
        assert!(all.is_empty());
    }
}
