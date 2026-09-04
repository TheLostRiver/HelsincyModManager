mod analysis;
mod binary;
mod catalog;
mod content_transform;
mod family;
mod mrl3_reference;
mod mrl3_transform;
mod part_rename;
mod path;
mod replacement;

pub use analysis::{
    analyze_mhw_weapon_assets, WeaponAnalysisError, WeaponAnalysisWarning, WeaponModelPair,
    WeaponSourceAsset, WeaponSourceClosure,
};
pub use binary::{
    preflight_mhw_weapon_mod3, preflight_mhw_weapon_model_pair, preflight_mhw_weapon_mrl3,
    WeaponBinaryError, WeaponMod3Preflight, WeaponModelPairPreflight, WeaponMrl3Preflight,
    MHW_WEAPON_BINARY_MAX_BYTES,
};
pub use catalog::{
    MhwWeaponCatalogSource, WeaponCatalogSourceError, WeaponTargetMetadata, WeaponTargetStatus,
    MHW_WEAPON_CATALOG_SOURCE_SCHEMA_VERSION,
};
pub use content_transform::{
    build_mhw_weapon_mrl3_transform_invocation, MhwWeaponMrl3TexturePathTransformer,
};
pub use family::{
    WeaponFamily, WeaponFamilyError, WeaponMainId, WeaponPartId, WeaponPartRole,
    WeaponSecondaryPart,
};
pub use mrl3_transform::{
    transform_mhw_weapon_mrl3_texture_paths, WeaponMrl3TransformOutput, WeaponMrl3TransformReport,
    MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID, MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_VERSION,
};
pub use path::{WeaponModelAssetKind, WeaponModelAssetPath, WeaponPathError, WeaponResourceRoot};
pub use replacement::{MhwReplacementAdapter, MhwReplacementCatalog};
