mod catalog;
mod path;
mod retarget;
mod slot_rename;

pub(crate) use catalog::resolve_target_allowing_legacy_ids;
pub use catalog::{normalize_armor_display_text, normalize_armor_search_text, MhwArmorCatalog};
/// `#356`：治理 validator 也要按同一份变体清单校验，不再各自硬编码。
pub(crate) use path::ArmorEquipFamily;
pub use path::{ArmorPathError, ArmorResourcePath};
pub use retarget::MhwArmorReplacementAdapter;
