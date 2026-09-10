use super::{
    AnalyzeImportedReplacementRequest, ReplacementWorkflowError, ReplacementWorkflowService,
};
use crate::InstallManifestQueryService;
use hmm_core::{
    GameId, ModId, ProfileId, ReplacementAnalysis, ReplacementBindingSnapshot, ReplacementSource,
    ReplacementTarget,
};
use hmm_ports::ReplacementCatalogProvider;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementSummaryItem {
    pub id: String,
    pub kind: String,
    pub internal_id: String,
    pub display_names: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModReplacementSummary {
    pub game_id: GameId,
    pub mod_id: ModId,
    pub package_id: String,
    pub sources: Vec<ReplacementSummaryItem>,
    /// None means no profile or unverified install facts, never "no retarget".
    pub installed_targets: Option<Vec<ReplacementSummaryItem>>,
}

impl ReplacementWorkflowService {
    pub fn describe_sources(&self, analysis: &ReplacementAnalysis) -> Vec<ReplacementSummaryItem> {
        let targets = self
            .list_targets(analysis.game_id(), None)
            .unwrap_or_default();
        analysis
            .sources()
            .iter()
            .map(|source| describe_source(source, &targets))
            .collect()
    }

    /// Display-only query: multiple sources/bindings do not grant single-target write authority.
    pub fn replacement_summary(
        &self,
        request: AnalyzeImportedReplacementRequest,
        profile_id: Option<&ProfileId>,
    ) -> Result<ModReplacementSummary, ReplacementWorkflowError> {
        let resolved = self.resolve_imported_replacement(&request.game_id, &request.mod_id)?;
        let catalog = self.catalog_for(&request.game_id).ok();
        let targets = self
            .list_targets(&request.game_id, None)
            .unwrap_or_default();
        let sources = resolved
            .analysis
            .sources()
            .iter()
            .map(|source| describe_source(source, &targets))
            .collect();
        let query = InstallManifestQueryService::new(Arc::clone(&self.install_manifests));
        let installed_targets = profile_id.and_then(|profile_id| {
            query
                .query_installed_replacement_bindings_for_display(profile_id, &request.mod_id)
                .ok()
                .map(|bindings| {
                    let mut items = BTreeMap::new();
                    for binding in bindings {
                        let item = describe_binding(&request.game_id, &binding, catalog.as_deref());
                        items.insert(item.id.clone(), item);
                    }
                    items.into_values().collect()
                })
        });
        Ok(ModReplacementSummary {
            game_id: request.game_id,
            mod_id: request.mod_id,
            package_id: resolved.package_id,
            sources,
            installed_targets,
        })
    }
}

fn describe_source(
    source: &ReplacementSource,
    targets: &[ReplacementTarget],
) -> ReplacementSummaryItem {
    let matching = targets.iter().filter(|target| {
        target.game_id() == source.game_id()
            && target.target_type() == source.source_type()
            && target.internal_id() == source.internal_id()
            && target
                .metadata()
                .get("path_family")
                .and_then(serde_json::Value::as_str)
                == Some(source.path_family())
    });
    ReplacementSummaryItem {
        id: source.id().as_str().to_owned(),
        kind: source.source_type().as_str().to_owned(),
        internal_id: source.internal_id().to_owned(),
        display_names: unique_names(matching),
    }
}

fn describe_binding(
    game_id: &GameId,
    binding: &ReplacementBindingSnapshot,
    catalog: Option<&dyn ReplacementCatalogProvider>,
) -> ReplacementSummaryItem {
    // 旧绑定 ID 的解析仍归 adapter；解析成功也必须复核快照的类型、编号与 path family。
    let target = catalog.and_then(|catalog| {
        catalog
            .find_replacement_target(binding.binding().target_id())
            .ok()
    });
    let matching = target.iter().filter(|target| {
        target.game_id() == game_id
            && target.target_type() == binding.retarget_kind()
            && target.internal_id() == binding.target_internal_id()
            && target
                .metadata()
                .get("path_family")
                .and_then(serde_json::Value::as_str)
                == Some(binding.target_path_family())
    });
    ReplacementSummaryItem {
        id: binding.binding().target_id().as_str().to_owned(),
        kind: binding.retarget_kind().as_str().to_owned(),
        internal_id: binding.target_internal_id().to_owned(),
        display_names: unique_names(matching),
    }
}

fn unique_names<'a>(
    mut matching: impl Iterator<Item = &'a ReplacementTarget>,
) -> BTreeMap<String, String> {
    match (matching.next(), matching.next()) {
        (Some(target), None) => target.display_name().clone().into(),
        _ => BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        LocalizedText, ReplacementBinding, ReplacementBindingId, ReplacementCatalog,
        ReplacementCatalogVersion, ReplacementSourceId, ReplacementTargetId, ReplacementTargetKind,
    };
    use hmm_ports::{ReplacementCatalogError, ReplacementCatalogResult};

    struct DisplayCatalog {
        target: ReplacementTarget,
        lookup_id: ReplacementTargetId,
    }

    impl ReplacementCatalogProvider for DisplayCatalog {
        fn game_id(&self) -> GameId {
            GameId::mhw()
        }
        fn replacement_catalog(&self) -> ReplacementCatalogResult<ReplacementCatalog> {
            Ok(ReplacementCatalog::new(
                ReplacementCatalogVersion::parse("fixture-v1").unwrap(),
                GameId::mhw(),
                vec![self.target.clone()],
            )
            .unwrap())
        }
        fn find_replacement_target(
            &self,
            id: &ReplacementTargetId,
        ) -> ReplacementCatalogResult<ReplacementTarget> {
            if id == &self.lookup_id {
                Ok(self.target.clone())
            } else {
                Err(ReplacementCatalogError::TargetNotFound {
                    target_id: id.clone(),
                })
            }
        }
        fn search_replacement_targets(
            &self,
            _query: &str,
        ) -> ReplacementCatalogResult<Vec<ReplacementTarget>> {
            Ok(vec![])
        }
    }

    fn catalog(target: ReplacementTarget) -> DisplayCatalog {
        DisplayCatalog {
            target,
            lookup_id: ReplacementTargetId::parse("target").unwrap(),
        }
    }

    fn source(kind: &str, family: &str) -> ReplacementSource {
        ReplacementSource::new(
            ReplacementSourceId::parse("fixture-source").unwrap(),
            GameId::mhw(),
            ReplacementTargetKind::parse(kind).unwrap(),
            "001",
            family,
            true,
        )
        .unwrap()
    }

    fn target(id: &str, kind: &str, family: &str) -> ReplacementTarget {
        ReplacementTarget::new(
            ReplacementTargetId::parse(id).unwrap(),
            GameId::mhw(),
            ReplacementTargetKind::parse(kind).unwrap(),
            LocalizedText::new(BTreeMap::from([
                ("en".to_owned(), format!("Fixture {id}")),
                ("zh_cn".to_owned(), format!("Fixture ZH {id}")),
            ]))
            .unwrap(),
            vec![],
            "001",
            BTreeMap::from([("path_family".to_owned(), serde_json::json!(family))]),
        )
        .unwrap()
    }

    #[test]
    fn source_names_match_kind_family_and_internal_id_exactly() {
        let correct = target("correct", "weapon", "family-a");
        let item = describe_source(
            &source("weapon", "family-a"),
            &[
                target("wrong-kind", "armor", "family-a"),
                target("wrong-family", "weapon", "family-b"),
                correct.clone(),
            ],
        );
        assert_eq!(
            item.display_names,
            BTreeMap::from(correct.display_name().clone())
        );
        assert_eq!(item.internal_id, "001");
        assert_eq!(item.kind, "weapon");
    }

    #[test]
    fn armor_sources_keep_their_name_and_identifier() {
        let item = describe_source(
            &source("armor", "family-a"),
            &[target("armor", "armor", "family-a")],
        );
        assert_eq!(item.display_names["en"], "Fixture armor");
        assert_eq!(item.internal_id, "001");
    }

    #[test]
    fn source_names_never_match_another_internal_id() {
        let source = ReplacementSource::new(
            ReplacementSourceId::parse("source").unwrap(),
            GameId::mhw(),
            ReplacementTargetKind::parse("armor").unwrap(),
            "002",
            "family-a",
            true,
        )
        .unwrap();
        assert!(
            describe_source(&source, &[target("target", "armor", "family-a")])
                .display_names
                .is_empty()
        );
    }

    #[test]
    fn missing_catalog_name_keeps_the_source_identifier() {
        let item = describe_source(&source("armor", "family-a"), &[]);
        assert_eq!(item.internal_id, "001");
        assert!(item.display_names.is_empty());
    }

    #[test]
    fn ambiguous_catalog_matches_never_pick_an_arbitrary_name() {
        let item = describe_source(
            &source("armor", "family-a"),
            &[
                target("first", "armor", "family-a"),
                target("second", "armor", "family-a"),
            ],
        );
        assert!(item.display_names.is_empty());
    }

    #[test]
    fn installed_names_require_snapshot_identity_not_only_catalog_id() {
        let binding = ReplacementBindingSnapshot::new(
            ReplacementBinding::new(
                ReplacementBindingId::parse("binding").unwrap(),
                ModId::new("mod"),
                ProfileId::new("profile"),
                ReplacementSourceId::parse("source").unwrap(),
                ReplacementTargetId::parse("target").unwrap(),
                1,
            )
            .unwrap(),
            None,
            "000",
            "001",
            "family-a",
            "family-b",
            ReplacementTargetKind::parse("armor").unwrap(),
        )
        .unwrap();
        let good = describe_binding(
            &GameId::mhw(),
            &binding,
            Some(&catalog(target("target", "armor", "family-b"))),
        );
        assert_eq!(good.display_names["en"], "Fixture target");
        let drifted = describe_binding(
            &GameId::mhw(),
            &binding,
            Some(&catalog(target("target", "armor", "family-a"))),
        );
        assert_eq!(drifted.internal_id, "001");
        assert!(drifted.display_names.is_empty());

        let legacy = describe_binding(
            &GameId::mhw(),
            &binding,
            Some(&catalog(target("canonical", "armor", "family-b"))),
        );
        assert_eq!(legacy.display_names["en"], "Fixture canonical");
        assert_eq!(legacy.id, "target");
        assert_eq!(legacy.internal_id, "001");

        let unknown = DisplayCatalog {
            target: target("canonical", "armor", "family-b"),
            lookup_id: ReplacementTargetId::parse("different").unwrap(),
        };
        assert!(describe_binding(&GameId::mhw(), &binding, Some(&unknown))
            .display_names
            .is_empty());
        assert!(describe_binding(&GameId::mhw(), &binding, None)
            .display_names
            .is_empty());
    }
}
