use hmm_core::{
    GameId, LocalizedText, ReplacementCatalog, ReplacementCatalogVersion, ReplacementTarget,
    ReplacementTargetId, ReplacementTargetKind,
};
use hmm_ports::{ReplacementCatalogError, ReplacementCatalogProvider, ReplacementCatalogResult};
use std::collections::BTreeMap;

struct FakeCatalogProvider;

impl FakeCatalogProvider {
    fn target() -> ReplacementTarget {
        ReplacementTarget::new(
            ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target id"),
            GameId::mhw(),
            ReplacementTargetKind::parse("armor").expect("target kind"),
            LocalizedText::new(BTreeMap::from([(
                "en".to_owned(),
                "Fatalis Alpha +".to_owned(),
            )]))
            .expect("localized text"),
            vec!["fatalis".to_owned()],
            "pl129_0000",
            BTreeMap::new(),
        )
        .expect("target")
    }
}

impl ReplacementCatalogProvider for FakeCatalogProvider {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn replacement_catalog(&self) -> ReplacementCatalogResult<ReplacementCatalog> {
        ReplacementCatalog::new(
            ReplacementCatalogVersion::parse("fake-v1").expect("version"),
            GameId::mhw(),
            vec![Self::target()],
        )
        .map_err(|_| ReplacementCatalogError::CatalogInvalid)
    }

    fn search_replacement_targets(
        &self,
        query: &str,
    ) -> ReplacementCatalogResult<Vec<ReplacementTarget>> {
        if query.eq_ignore_ascii_case("fatalis") {
            Ok(vec![Self::target()])
        } else {
            Ok(Vec::new())
        }
    }
}

#[test]
fn catalog_provider_finds_by_project_stable_target_id() {
    let provider = FakeCatalogProvider;
    let target_id = ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target id");

    let target = provider
        .find_replacement_target(&target_id)
        .expect("target should exist");

    assert_eq!(target.id(), &target_id);
    assert_eq!(target.internal_id(), "pl129_0000");
}

#[test]
fn catalog_provider_reports_stable_not_found_error() {
    let provider = FakeCatalogProvider;
    let target_id = ReplacementTargetId::parse("mhw:armor:missing").expect("target id");

    let error = provider
        .find_replacement_target(&target_id)
        .expect_err("target should be absent");

    assert_eq!(error, ReplacementCatalogError::TargetNotFound { target_id });
    assert_eq!(
        error.to_string(),
        "replacement target not found: mhw:armor:missing"
    );
}

#[test]
fn catalog_provider_keeps_game_specific_search_behind_the_port() {
    let provider = FakeCatalogProvider;

    let results = provider
        .search_replacement_targets("FATALIS")
        .expect("search");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id().as_str(), "mhw:armor:fatalis-alpha");
}
