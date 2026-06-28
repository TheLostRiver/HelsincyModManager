/// A user-defined category for organizing mods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i32,
    pub created_at: u128,
}

/// A lightweight label used in `ModLibraryItem` to present both user categories
/// (with color) and import-derived tags (color = `None`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryLabel {
    pub name: String,
    pub color: Option<String>,
}
