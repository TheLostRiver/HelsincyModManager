use anyhow::{anyhow, Context, Result};
use hmm_core::Profile;
use hmm_ports::ProfileRepository;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub struct SqliteProfileRepository {
    db: Arc<Mutex<Connection>>,
}

impl SqliteProfileRepository {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| anyhow!("database lock poisoned"))
    }

    fn row_to_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<Profile> {
        Ok(Profile {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            is_active: row.get::<_, i64>(3)? != 0,
            created_at: row.get::<_, i64>(4)? as u128,
            updated_at: row.get::<_, i64>(5)? as u128,
        })
    }
}

impl ProfileRepository for SqliteProfileRepository {
    fn get(&self, profile_id: &str) -> Result<Option<Profile>> {
        let conn = self.lock_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT profile_id, name, description, is_active, created_at, updated_at
                 FROM profiles WHERE profile_id = ?1",
            )
            .context("failed to prepare get profile query")?;

        let result = stmt.query_row(rusqlite::params![profile_id], Self::row_to_profile);

        match result {
            Ok(profile) => Ok(Some(profile)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error).context("failed to get profile"),
        }
    }

    fn save(&self, profile: &Profile) -> Result<()> {
        let conn = self.lock_db()?;
        conn.execute(
            "INSERT INTO profiles
                (profile_id, name, description, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(profile_id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                is_active = excluded.is_active,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at",
            rusqlite::params![
                profile.id,
                profile.name,
                profile.description,
                if profile.is_active { 1 } else { 0 },
                profile.created_at as i64,
                profile.updated_at as i64,
            ],
        )
        .context("failed to save profile")?;
        Ok(())
    }

    fn delete(&self, profile_id: &str) -> Result<()> {
        let conn = self.lock_db()?;
        conn.execute(
            "DELETE FROM profiles WHERE profile_id = ?1",
            rusqlite::params![profile_id],
        )
        .context("failed to delete profile")?;
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<Profile>> {
        let conn = self.lock_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT profile_id, name, description, is_active, created_at, updated_at
                 FROM profiles ORDER BY is_active DESC, created_at ASC, name ASC",
            )
            .context("failed to prepare list profiles query")?;

        let rows = stmt
            .query_map([], Self::row_to_profile)
            .context("failed to list profiles")?;

        let mut profiles = Vec::new();
        for row in rows {
            profiles.push(row.context("failed to read profile row")?);
        }
        Ok(profiles)
    }

    fn get_active(&self) -> Result<Option<Profile>> {
        let conn = self.lock_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT profile_id, name, description, is_active, created_at, updated_at
                 FROM profiles WHERE is_active = 1 LIMIT 1",
            )
            .context("failed to prepare get active profile query")?;

        let result = stmt.query_row([], Self::row_to_profile);

        match result {
            Ok(profile) => Ok(Some(profile)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error).context("failed to get active profile"),
        }
    }

    fn set_active(&self, profile_id: &str, updated_at: u128) -> Result<()> {
        let conn = self.lock_db()?;
        let tx = conn
            .unchecked_transaction()
            .context("failed to begin profile activation transaction")?;

        let exists = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE profile_id = ?1)",
                rusqlite::params![profile_id],
                |row| row.get::<_, i64>(0),
            )
            .context("failed to check profile existence")?
            != 0;
        if !exists {
            return Err(anyhow!("profile not found: {profile_id}"));
        }

        tx.execute("UPDATE profiles SET is_active = 0 WHERE is_active = 1", [])
            .context("failed to clear active profile")?;
        let affected = tx
            .execute(
                "UPDATE profiles SET is_active = 1, updated_at = ?2 WHERE profile_id = ?1",
                rusqlite::params![profile_id, updated_at as i64],
            )
            .context("failed to set active profile")?;
        if affected != 1 {
            return Err(anyhow!("profile not found: {profile_id}"));
        }

        tx.commit()
            .context("failed to commit profile activation")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::open_database;

    fn test_repo() -> SqliteProfileRepository {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("test.db");
        let conn = open_database(&db_path).unwrap();
        std::mem::forget(temp);
        SqliteProfileRepository::new(Arc::new(Mutex::new(conn)))
    }

    fn sample_profile(id: &str, name: &str, is_active: bool) -> Profile {
        Profile {
            id: id.to_owned(),
            name: name.to_owned(),
            description: None,
            is_active,
            created_at: 1000,
            updated_at: 1000,
        }
    }

    #[test]
    fn default_profile_exists() {
        let repo = test_repo();
        let active = repo.get_active().unwrap().expect("active profile");

        assert_eq!(active.id, "default");
        assert_eq!(active.name, "Default");
        assert!(active.is_active);
    }

    #[test]
    fn save_and_get_round_trips() {
        let repo = test_repo();
        let mut profile = sample_profile("profile-1", "Hunt", false);
        profile.description = Some("Testing".to_owned());

        repo.save(&profile).unwrap();
        let loaded = repo.get("profile-1").unwrap().expect("profile exists");

        assert_eq!(loaded.name, "Hunt");
        assert_eq!(loaded.description.as_deref(), Some("Testing"));
        assert!(!loaded.is_active);
    }

    #[test]
    fn set_active_deactivates_existing_active_profile() {
        let repo = test_repo();
        repo.save(&sample_profile("profile-2", "Second", false))
            .unwrap();

        repo.set_active("profile-2", 5000).unwrap();

        assert!(!repo.get("default").unwrap().unwrap().is_active);
        let active = repo.get_active().unwrap().expect("active profile");
        assert_eq!(active.id, "profile-2");
        assert_eq!(active.updated_at, 5000);
    }

    #[test]
    fn set_active_unknown_profile_keeps_existing_active_profile() {
        let repo = test_repo();

        let result = repo.set_active("missing", 5000);

        assert!(result.is_err());
        let active = repo.get_active().unwrap().expect("active profile");
        assert_eq!(active.id, "default");
        assert!(active.is_active);
    }

    #[test]
    fn list_all_orders_active_profile_first() {
        let repo = test_repo();
        repo.save(&sample_profile("profile-2", "Second", false))
            .unwrap();
        repo.set_active("profile-2", 5000).unwrap();

        let profiles = repo.list_all().unwrap();

        assert_eq!(profiles[0].id, "profile-2");
    }
}
