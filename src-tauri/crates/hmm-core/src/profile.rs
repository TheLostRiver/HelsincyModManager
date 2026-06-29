pub const DEFAULT_PROFILE_ID: &str = "default";

/// A user-editable mod loadout scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: u128,
    pub updated_at: u128,
}
