use anyhow::Result;
use hmm_core::Profile;

pub trait ProfileRepository: Send + Sync {
    fn get(&self, profile_id: &str) -> Result<Option<Profile>>;
    fn save(&self, profile: &Profile) -> Result<()>;
    fn delete(&self, profile_id: &str) -> Result<()>;
    fn list_all(&self) -> Result<Vec<Profile>>;
    fn get_active(&self) -> Result<Option<Profile>>;
    fn set_active(&self, profile_id: &str, updated_at: u128) -> Result<()>;
}
