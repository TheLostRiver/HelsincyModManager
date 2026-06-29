use anyhow::{bail, Result};
use hmm_core::{Profile, DEFAULT_PROFILE_ID};
use hmm_ports::{AppClock, ProfileRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateProfileRequest {
    pub name: String,
    pub description: Option<String>,
}

pub struct UpdateProfileRequest {
    pub profile_id: String,
    pub name: Option<String>,
    pub description: Option<Option<String>>,
}

pub struct ProfileService {
    profile_repository: Arc<dyn ProfileRepository>,
    clock: Arc<dyn AppClock>,
}

impl ProfileService {
    pub fn new(
        profile_repository: Arc<dyn ProfileRepository>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            profile_repository,
            clock,
        }
    }

    pub fn create_profile(&self, request: CreateProfileRequest) -> Result<String> {
        let name = normalize_required_name(request.name)?;
        let description = normalize_optional_string(request.description);
        let now = self.clock.now_unix_millis()?;
        let id = Uuid::new_v4().to_string();

        let profile = Profile {
            id: id.clone(),
            name,
            description,
            is_active: false,
            created_at: now,
            updated_at: now,
        };

        self.profile_repository.save(&profile)?;
        Ok(id)
    }

    pub fn update_profile(&self, request: UpdateProfileRequest) -> Result<()> {
        let existing = self
            .profile_repository
            .get(&request.profile_id)?
            .ok_or_else(|| anyhow::anyhow!("profile not found: {}", request.profile_id))?;

        let name = match request.name {
            Some(name) => normalize_required_name(name)?,
            None => existing.name,
        };
        let description = match request.description {
            Some(description) => normalize_optional_string(description),
            None => existing.description,
        };
        let now = self.clock.now_unix_millis()?;

        self.profile_repository.save(&Profile {
            id: existing.id,
            name,
            description,
            is_active: existing.is_active,
            created_at: existing.created_at,
            updated_at: now,
        })
    }

    pub fn delete_profile(&self, profile_id: &str) -> Result<()> {
        if profile_id == DEFAULT_PROFILE_ID {
            bail!("default profile cannot be deleted");
        }

        let existing = self
            .profile_repository
            .get(profile_id)?
            .ok_or_else(|| anyhow::anyhow!("profile not found: {profile_id}"))?;
        if existing.is_active {
            bail!("active profile cannot be deleted");
        }

        self.profile_repository.delete(profile_id)
    }

    pub fn list_profiles(&self) -> Result<Vec<Profile>> {
        self.profile_repository.list_all()
    }

    pub fn get_active_profile(&self) -> Result<Profile> {
        self.profile_repository
            .get_active()?
            .ok_or_else(|| anyhow::anyhow!("active profile not found"))
    }

    pub fn set_active_profile(&self, profile_id: &str) -> Result<()> {
        self.profile_repository
            .get(profile_id)?
            .ok_or_else(|| anyhow::anyhow!("profile not found: {profile_id}"))?;
        let now = self.clock.now_unix_millis()?;
        self.profile_repository.set_active(profile_id, now)
    }
}

fn normalize_required_name(value: String) -> Result<String> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        bail!("profile name must not be empty");
    }
    Ok(value)
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}
