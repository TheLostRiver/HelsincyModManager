use anyhow::{Context, Result};
use hmm_core::ProfileId;
use hmm_ports::{
    normalize_mod_library_query_key, ModLibraryProfileProjection, ModLibraryProfileProjectionState,
    ModLibraryProjectionReadiness, ModLibraryProjectionRecord, ModLibraryProjectionRepository,
    ModLibraryProjectionSnapshot, ModLibraryProjectionState, MOD_LIBRARY_PROJECTION_SCHEMA_VERSION,
    MOD_LIBRARY_QUERY_KEY_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SqliteModLibraryProjectionRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteModLibraryProjectionRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    fn lock_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| anyhow::anyhow!("mod library projection database lock poisoned"))
    }
}

impl ModLibraryProjectionRepository for SqliteModLibraryProjectionRepository {
    fn state(&self) -> Result<ModLibraryProjectionState> {
        let conn = self.lock_connection()?;
        read_state(&conn)
    }

    fn mark_dirty(&self, observed_source_fingerprint: Option<&str>) -> Result<()> {
        let conn = self.lock_connection()?;
        conn.execute(
            "UPDATE mod_library_projection_state
             SET readiness = 'dirty',
                 source_fingerprint = COALESCE(?1, source_fingerprint),
                 updated_at = ?2
             WHERE singleton_id = 1",
            params![observed_source_fingerprint, now_unix_millis()?],
        )
        .context("failed to mark Mod library projection dirty")?;
        Ok(())
    }

    fn rebuild(
        &self,
        snapshot: &ModLibraryProjectionSnapshot,
    ) -> Result<ModLibraryProjectionState> {
        self.mark_dirty(Some(&snapshot.source_fingerprint))?;
        let prepared = prepare_snapshot(snapshot)?;
        let mut conn = self.lock_connection()?;
        let transaction = conn
            .transaction()
            .context("failed to start Mod library projection rebuild")?;
        let current = read_state(&transaction)?;
        let generation = current
            .generation
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Mod library projection generation overflow"))?;

        transaction
            .execute("DELETE FROM mod_library_projection_profile_status", [])
            .context("failed to clear Mod library projection profile status")?;
        transaction
            .execute("DELETE FROM mod_library_projection_profile_generations", [])
            .context("failed to clear Mod library projection profile generations")?;
        transaction
            .execute("DELETE FROM mod_library_projection_labels", [])
            .context("failed to clear Mod library projection labels")?;
        transaction
            .execute("DELETE FROM mod_library_projection_items", [])
            .context("failed to clear Mod library projection items")?;

        for record in &prepared.records {
            insert_record(&transaction, generation, record)?;
        }
        for profile in &prepared.profiles {
            insert_profile(&transaction, generation, profile)?;
        }
        transaction
            .execute(
                "UPDATE mod_library_projection_state
                 SET schema_version = ?1,
                     key_version = ?2,
                     generation = ?3,
                     source_fingerprint = ?4,
                     readiness = 'complete',
                     updated_at = ?5
                 WHERE singleton_id = 1",
                params![
                    MOD_LIBRARY_PROJECTION_SCHEMA_VERSION,
                    MOD_LIBRARY_QUERY_KEY_VERSION,
                    sqlite_i64(generation)?,
                    prepared.source_fingerprint,
                    now_unix_millis()?
                ],
            )
            .context("failed to publish Mod library projection generation")?;
        transaction
            .commit()
            .context("failed to commit Mod library projection rebuild")?;
        drop(conn);
        self.state()
    }

    fn profile_state(
        &self,
        profile_id: &ProfileId,
    ) -> Result<Option<ModLibraryProfileProjectionState>> {
        let conn = self.lock_connection()?;
        read_profile_state(&conn, profile_id)
    }

    fn mark_profile_dirty(
        &self,
        profile_id: &ProfileId,
        observed_source_fingerprint: Option<&str>,
    ) -> Result<()> {
        let conn = self.lock_connection()?;
        let existing_generation = read_profile_state(&conn, profile_id)?
            .map(|state| state.generation)
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO mod_library_projection_profile_generations (
                 profile_id, generation, source_fingerprint, readiness, updated_at
             ) VALUES (?1, ?2, ?3, 'dirty', ?4)
             ON CONFLICT(profile_id) DO UPDATE SET
                 source_fingerprint = COALESCE(excluded.source_fingerprint, source_fingerprint),
                 readiness = 'dirty',
                 updated_at = excluded.updated_at",
            params![
                profile_id.as_str(),
                sqlite_i64(existing_generation)?,
                observed_source_fingerprint,
                now_unix_millis()?
            ],
        )
        .context("failed to mark Mod library profile projection dirty")?;
        Ok(())
    }

    fn replace_profile(
        &self,
        projection: &ModLibraryProfileProjection,
    ) -> Result<ModLibraryProfileProjectionState> {
        self.mark_profile_dirty(&projection.profile_id, Some(&projection.source_fingerprint))?;
        let prepared = prepare_profile(projection, None)?;
        let mut conn = self.lock_connection()?;
        let transaction = conn
            .transaction()
            .context("failed to start Mod library profile projection update")?;
        let generation = read_profile_state(&transaction, &projection.profile_id)?
            .map(|state| state.generation)
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Mod library profile generation overflow"))?;
        transaction
            .execute(
                "DELETE FROM mod_library_projection_profile_status WHERE profile_id = ?1",
                params![projection.profile_id.as_str()],
            )
            .context("failed to clear Mod library profile projection status")?;
        insert_profile(&transaction, generation, &prepared)?;
        transaction
            .commit()
            .context("failed to commit Mod library profile projection update")?;
        drop(conn);
        self.profile_state(&projection.profile_id)?
            .ok_or_else(|| anyhow::anyhow!("Mod library profile projection was not published"))
    }
}

struct PreparedSnapshot {
    source_fingerprint: String,
    records: Vec<PreparedRecord>,
    profiles: Vec<PreparedProfile>,
}

struct PreparedRecord {
    record: ModLibraryProjectionRecord,
    preview_image_json: String,
    normalized_name: String,
    normalized_author: String,
    normalized_labels: Vec<String>,
}

struct PreparedProfile {
    projection: ModLibraryProfileProjection,
}

fn prepare_snapshot(snapshot: &ModLibraryProjectionSnapshot) -> Result<PreparedSnapshot> {
    ensure_fingerprint(&snapshot.source_fingerprint)?;
    let mut mod_ids = HashSet::with_capacity(snapshot.records.len());
    let mut revision_ids = HashSet::with_capacity(snapshot.records.len());
    let mut package_ids = HashSet::with_capacity(snapshot.records.len());
    let records = snapshot
        .records
        .iter()
        .map(|record| {
            anyhow::ensure!(
                mod_ids.insert(record.mod_id.as_str().to_owned()),
                "Mod library projection contains duplicate Mod ids"
            );
            anyhow::ensure!(
                revision_ids.insert(record.display_revision_id.as_str().to_owned()),
                "Mod library projection contains duplicate display revision ids"
            );
            anyhow::ensure!(
                package_ids.insert(record.package_id.clone()),
                "Mod library projection contains duplicate package ids"
            );
            prepare_record(record)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut profile_ids = HashSet::with_capacity(snapshot.profiles.len());
    let profiles = snapshot
        .profiles
        .iter()
        .map(|profile| {
            anyhow::ensure!(
                profile_ids.insert(profile.profile_id.as_str().to_owned()),
                "Mod library projection contains duplicate profile ids"
            );
            prepare_profile(profile, Some(&mod_ids))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(PreparedSnapshot {
        source_fingerprint: snapshot.source_fingerprint.clone(),
        records,
        profiles,
    })
}

fn prepare_record(record: &ModLibraryProjectionRecord) -> Result<PreparedRecord> {
    anyhow::ensure!(
        !record.mod_id.as_str().is_empty()
            && !record.display_revision_id.as_str().is_empty()
            && !record.package_id.is_empty()
            && !record.display_name.is_empty(),
        "Mod library projection record is incomplete"
    );
    anyhow::ensure!(
        record.labels.len() <= 1_000,
        "Mod library projection record has too many labels"
    );
    let preview_image_json = serde_json::to_string(&record.preview_image)
        .context("failed to serialize Mod library projection preview image")?;
    let normalized_labels = record
        .labels
        .iter()
        .map(|label| {
            anyhow::ensure!(
                !label.name.is_empty(),
                "Mod library projection label name is empty"
            );
            Ok(normalize_mod_library_query_key(&label.name))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(PreparedRecord {
        record: record.clone(),
        preview_image_json,
        normalized_name: normalize_mod_library_query_key(&record.display_name),
        normalized_author: record
            .author
            .as_deref()
            .map(normalize_mod_library_query_key)
            .unwrap_or_default(),
        normalized_labels,
    })
}

fn prepare_profile(
    projection: &ModLibraryProfileProjection,
    known_mod_ids: Option<&HashSet<String>>,
) -> Result<PreparedProfile> {
    ensure_fingerprint(&projection.source_fingerprint)?;
    let mut status_ids = HashSet::with_capacity(projection.statuses.len());
    for status in &projection.statuses {
        anyhow::ensure!(
            status_ids.insert(status.mod_id.as_str().to_owned()),
            "Mod library profile projection contains duplicate Mod ids"
        );
        if let Some(known_mod_ids) = known_mod_ids {
            anyhow::ensure!(
                known_mod_ids.contains(status.mod_id.as_str()),
                "Mod library profile projection references an unknown Mod"
            );
        }
        anyhow::ensure!(
            status.backup_count <= status.managed_file_count,
            "Mod library profile projection backup count exceeds managed files"
        );
    }
    Ok(PreparedProfile {
        projection: projection.clone(),
    })
}

fn ensure_fingerprint(fingerprint: &str) -> Result<()> {
    anyhow::ensure!(
        !fingerprint.is_empty() && fingerprint.len() <= 256,
        "Mod library projection source fingerprint is invalid"
    );
    Ok(())
}

fn insert_record(
    transaction: &Transaction<'_>,
    generation: u64,
    prepared: &PreparedRecord,
) -> Result<()> {
    let record = &prepared.record;
    transaction
        .execute(
            "INSERT INTO mod_library_projection_items (
                 mod_id, generation, display_revision_id, package_id, display_name,
                 author, version_label, size_label, preview_image_json,
                 normalized_name, normalized_author
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                record.mod_id.as_str(),
                sqlite_i64(generation)?,
                record.display_revision_id.as_str(),
                record.package_id,
                record.display_name,
                record.author,
                record.version_label,
                record.size_label,
                prepared.preview_image_json,
                prepared.normalized_name,
                prepared.normalized_author
            ],
        )
        .context("failed to insert Mod library projection item")?;
    for (ordinal, (label, normalized_name)) in record
        .labels
        .iter()
        .zip(&prepared.normalized_labels)
        .enumerate()
    {
        transaction
            .execute(
                "INSERT INTO mod_library_projection_labels (
                     mod_id, ordinal, category_id, name, color, normalized_name
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    record.mod_id.as_str(),
                    i64::try_from(ordinal)
                        .context("label ordinal does not fit in SQLite integer")?,
                    label.category_id,
                    label.name,
                    label.color,
                    normalized_name
                ],
            )
            .context("failed to insert Mod library projection label")?;
    }
    Ok(())
}

fn insert_profile(
    transaction: &Transaction<'_>,
    generation: u64,
    prepared: &PreparedProfile,
) -> Result<()> {
    let projection = &prepared.projection;
    transaction
        .execute(
            "INSERT INTO mod_library_projection_profile_generations (
                 profile_id, generation, source_fingerprint, readiness, updated_at
             ) VALUES (?1, ?2, ?3, 'complete', ?4)
             ON CONFLICT(profile_id) DO UPDATE SET
                 generation = excluded.generation,
                 source_fingerprint = excluded.source_fingerprint,
                 readiness = excluded.readiness,
                 updated_at = excluded.updated_at",
            params![
                projection.profile_id.as_str(),
                sqlite_i64(generation)?,
                projection.source_fingerprint,
                now_unix_millis()?
            ],
        )
        .context("failed to publish Mod library profile generation")?;
    for status in &projection.statuses {
        transaction
            .execute(
                "INSERT INTO mod_library_projection_profile_status (
                     profile_id, mod_id, profile_generation, status,
                     managed_file_count, backup_count
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    projection.profile_id.as_str(),
                    status.mod_id.as_str(),
                    sqlite_i64(generation)?,
                    status.status.as_str(),
                    sqlite_i64(status.managed_file_count)?,
                    sqlite_i64(status.backup_count)?
                ],
            )
            .context("failed to insert Mod library profile status")?;
    }
    Ok(())
}

fn read_state(conn: &Connection) -> Result<ModLibraryProjectionState> {
    conn.query_row(
        "SELECT schema_version, key_version, generation, source_fingerprint, readiness
         FROM mod_library_projection_state WHERE singleton_id = 1",
        [],
        |row| {
            let readiness: String = row.get(4)?;
            Ok(ModLibraryProjectionState {
                schema_version: row.get(0)?,
                key_version: row.get(1)?,
                generation: u64::try_from(row.get::<_, i64>(2)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                source_fingerprint: row.get(3)?,
                readiness: parse_readiness(&readiness)?,
            })
        },
    )
    .context("failed to read Mod library projection state")
}

fn read_profile_state(
    conn: &Connection,
    profile_id: &ProfileId,
) -> Result<Option<ModLibraryProfileProjectionState>> {
    conn.query_row(
        "SELECT generation, source_fingerprint, readiness
         FROM mod_library_projection_profile_generations WHERE profile_id = ?1",
        params![profile_id.as_str()],
        |row| {
            let readiness: String = row.get(2)?;
            Ok(ModLibraryProfileProjectionState {
                profile_id: profile_id.clone(),
                generation: u64::try_from(row.get::<_, i64>(0)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                source_fingerprint: row.get(1)?,
                readiness: parse_readiness(&readiness)?,
            })
        },
    )
    .optional()
    .context("failed to read Mod library profile projection state")
}

fn parse_readiness(value: &str) -> rusqlite::Result<ModLibraryProjectionReadiness> {
    match value {
        "dirty" => Ok(ModLibraryProjectionReadiness::Dirty),
        "complete" => Ok(ModLibraryProjectionReadiness::Complete),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn sqlite_i64(value: u64) -> Result<i64> {
    i64::try_from(value).context("value does not fit in SQLite integer")
}

fn now_unix_millis() -> Result<i64> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_millis();
    sqlite_i64(u64::try_from(millis).context("system time does not fit in unsigned integer")?)
}
