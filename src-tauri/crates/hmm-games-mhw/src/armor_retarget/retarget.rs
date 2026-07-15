use super::path::is_valid_armor_slot;
use super::{ArmorPathError, ArmorResourcePath, MhwArmorCatalog};
use hmm_core::{
    GameId, PackageFileId, ReplacementAnalysis, ReplacementSource, ReplacementSourceId,
    ReplacementTarget, ReplacementTargetKind, ReplacementWarning, RetargetAction, RetargetPlan,
};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAdapterResult,
    ReplacementAnalysisRequest, ReplacementAsset, ReplacementCatalogError,
    ReplacementCatalogProvider, RetargetPlanRequest,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Default)]
pub struct MhwArmorReplacementAdapter;

#[derive(Debug, Clone)]
struct ParsedReplacementAsset {
    package_file_id: PackageFileId,
    path: ArmorResourcePath,
}

impl ReplacementAdapter for MhwArmorReplacementAdapter {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn analyze_replacement_assets(
        &self,
        request: ReplacementAnalysisRequest,
    ) -> ReplacementAdapterResult<ReplacementAnalysis> {
        self.ensure_game(&request.game_id)?;
        analyze_assets(&request.assets).map(|(analysis, _)| analysis)
    }

    fn build_retarget_plan(
        &self,
        request: RetargetPlanRequest,
    ) -> ReplacementAdapterResult<RetargetPlan> {
        self.ensure_game(&request.game_id)?;
        let (analysis, parsed_assets) = analyze_assets(&request.assets)?;
        let source = match analysis.sources() {
            [] => return Err(ReplacementAdapterError::UnrecognizedSourceSlot),
            [source] if source.is_supported() => source.clone(),
            _ => return Err(ReplacementAdapterError::AmbiguousSourceSlot),
        };

        if request.binding.source_id() != source.id() {
            return Err(ReplacementAdapterError::SourceBindingMismatch);
        }

        let target = find_target(request.binding.target_id())?;
        let target_path_family = target_path_family(&target)?;
        if target.target_type().as_str() != "armor"
            || !is_valid_armor_slot(target.internal_id())
            || source.path_family() != target_path_family
        {
            return Err(ReplacementAdapterError::UnsupportedReplacementTarget);
        }

        let actions = parsed_assets
            .into_iter()
            .filter(|asset| {
                asset.path.path_family() == source.path_family()
                    && asset.path.slot() == source.internal_id()
            })
            .map(|asset| build_action(asset, &source, &target, target_path_family))
            .collect::<ReplacementAdapterResult<Vec<_>>>()?;
        let warnings = (source.internal_id() == target.internal_id())
            .then_some(ReplacementWarning::SourceMatchesTarget)
            .into_iter()
            .collect();

        RetargetPlan::new(request.binding, source, actions, warnings)
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
    }
}

impl MhwArmorReplacementAdapter {
    fn ensure_game(&self, game_id: &GameId) -> ReplacementAdapterResult<()> {
        if game_id == &self.game_id() {
            Ok(())
        } else {
            Err(ReplacementAdapterError::UnsupportedGame)
        }
    }
}

fn analyze_assets(
    assets: &[ReplacementAsset],
) -> ReplacementAdapterResult<(ReplacementAnalysis, Vec<ParsedReplacementAsset>)> {
    let parsed_assets = parse_assets(assets)?;
    let mut grouped_sources = BTreeMap::<(String, String), bool>::new();
    for asset in &parsed_assets {
        grouped_sources.insert(
            (
                asset.path.path_family().to_owned(),
                asset.path.slot().to_owned(),
            ),
            asset.path.is_supported(),
        );
    }

    let sources = grouped_sources
        .into_iter()
        .map(|((path_family, slot), supported)| {
            let equip_family = path_family
                .rsplit('/')
                .next()
                .expect("MHW armor path families always contain an equip family");
            ReplacementSource::new(
                ReplacementSourceId::parse(format!("mhw:armor:{equip_family}:{slot}"))
                    .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?,
                GameId::mhw(),
                ReplacementTargetKind::parse("armor")
                    .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?,
                slot,
                path_family,
                supported,
            )
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
        })
        .collect::<ReplacementAdapterResult<Vec<_>>>()?;

    let mut warnings = Vec::new();
    if sources.is_empty() {
        warnings.push(ReplacementWarning::NoSupportedAssets);
    }
    if sources.len() > 1 {
        warnings.push(ReplacementWarning::MultipleSources);
    }
    if sources.iter().any(|source| !source.is_supported()) {
        warnings.push(ReplacementWarning::UnsupportedSource);
    }

    let analysis = ReplacementAnalysis::new(GameId::mhw(), sources, parsed_assets.len(), warnings)
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?;
    Ok((analysis, parsed_assets))
}

fn parse_assets(
    assets: &[ReplacementAsset],
) -> ReplacementAdapterResult<Vec<ParsedReplacementAsset>> {
    let mut parsed = Vec::new();
    for asset in assets {
        match ArmorResourcePath::parse(asset.relative_path()) {
            Ok(path) => parsed.push(ParsedReplacementAsset {
                package_file_id: asset.package_file_id().clone(),
                path,
            }),
            Err(ArmorPathError::NotArmorPath) => {}
            Err(ArmorPathError::UnsafePath) => {
                return Err(ReplacementAdapterError::UnsafeRetargetPath)
            }
            Err(ArmorPathError::MalformedArmorPath | ArmorPathError::InvalidSlot) => {
                return Err(ReplacementAdapterError::UnrecognizedSourceSlot)
            }
        }
    }
    Ok(parsed)
}

fn find_target(
    target_id: &hmm_core::ReplacementTargetId,
) -> ReplacementAdapterResult<ReplacementTarget> {
    MhwArmorCatalog
        .find_replacement_target(target_id)
        .map_err(|error| match error {
            ReplacementCatalogError::TargetNotFound { target_id } => {
                ReplacementAdapterError::TargetCatalogMissing { target_id }
            }
            ReplacementCatalogError::CatalogUnavailable
            | ReplacementCatalogError::CatalogInvalid
            | ReplacementCatalogError::UnsupportedSchemaVersion { .. } => {
                ReplacementAdapterError::TargetCatalogUnavailable
            }
        })
}

fn target_path_family(target: &ReplacementTarget) -> ReplacementAdapterResult<&str> {
    target
        .metadata()
        .get("path_family")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or(ReplacementAdapterError::UnsupportedReplacementTarget)
}

fn build_action(
    asset: ParsedReplacementAsset,
    source: &ReplacementSource,
    target: &ReplacementTarget,
    target_path_family: &str,
) -> ReplacementAdapterResult<RetargetAction> {
    let target_relative_path = asset
        .path
        .retarget(target.internal_id())
        .map_err(map_path_error)?;

    RetargetAction::new(
        asset.package_file_id,
        asset.path.normalized_path().clone(),
        target_relative_path,
        source.id().clone(),
        source.internal_id(),
        target.internal_id(),
        source.path_family(),
        target_path_family,
    )
    .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
}

fn map_path_error(error: ArmorPathError) -> ReplacementAdapterError {
    match error {
        ArmorPathError::UnsafePath => ReplacementAdapterError::UnsafeRetargetPath,
        ArmorPathError::NotArmorPath
        | ArmorPathError::MalformedArmorPath
        | ArmorPathError::InvalidSlot => ReplacementAdapterError::UnrecognizedSourceSlot,
    }
}
