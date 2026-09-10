use crate::{ArmorResourcePath, WeaponResourceRoot};
use hmm_core::{GameId, LocalizedText, ReplacementSource, ReplacementTarget, ReplacementTargetId};
use hmm_ports::{ReplacementCatalogError, ReplacementCatalogResult};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// 仅表示源资源本身；不会出现在可选目标目录，也不能解析为其他装备的目标。
pub(crate) fn original_target_identity(
    source: &ReplacementSource,
) -> ReplacementCatalogResult<ReplacementTarget> {
    if source.game_id() != &GameId::mhw() {
        return Err(ReplacementCatalogError::CatalogInvalid);
    }
    let root = format!("nativePC/{}/{}", source.path_family(), source.internal_id());
    let valid = match source.source_type().as_str() {
        "weapon" => WeaponResourceRoot::parse(&root).is_ok_and(|root| {
            root.main_id().as_str() == source.internal_id()
                && root.path_family() == source.path_family()
        }),
        "armor" => ArmorResourcePath::parse(&format!("{root}/resource.mod3")).is_ok_and(|path| {
            path.slot() == source.internal_id() && path.path_family() == source.path_family()
        }),
        _ => false,
    };
    if !valid {
        return Err(ReplacementCatalogError::CatalogInvalid);
    }
    let hash = Sha256::digest(
        format!(
            "{}|{}|{}",
            source.source_type().as_str(),
            source.path_family(),
            source.internal_id()
        )
        .as_bytes(),
    );
    ReplacementTarget::new(
        ReplacementTargetId::parse(format!("mhw:source-identity:{hash:x}"))
            .map_err(|_| ReplacementCatalogError::CatalogInvalid)?,
        GameId::mhw(),
        source.source_type().clone(),
        LocalizedText::new(BTreeMap::from([(
            "en".to_owned(),
            source.internal_id().to_owned(),
        )]))
        .map_err(|_| ReplacementCatalogError::CatalogInvalid)?,
        Vec::new(),
        source.internal_id(),
        BTreeMap::from([
            (
                "path_family".to_owned(),
                serde_json::json!(source.path_family()),
            ),
            ("identity_only".to_owned(), serde_json::json!(true)),
        ]),
    )
    .map_err(|_| ReplacementCatalogError::CatalogInvalid)
}
