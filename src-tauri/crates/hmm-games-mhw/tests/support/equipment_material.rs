#![allow(dead_code)]
use hmm_core::{GameId, ModId, PackageFileId, ProfileId, ReplacementBinding, ReplacementBindingId};
use hmm_games_mhw::{MhwReplacementAdapter, MhwReplacementCatalog};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAdapterResult,
    ReplacementAnalysisRequest, ReplacementAsset, ReplacementAssetContentReader,
    ReplacementCatalogProvider, RetargetPlanRequest,
};
use std::collections::BTreeMap;

pub fn material(references: &[&str]) -> Vec<u8> {
    let material_start = 40 + references.len() * 272;
    let mut bytes = vec![0xa5; material_start + 64];
    bytes[..40].fill(0);
    bytes[..4].copy_from_slice(b"MRL\0");
    bytes[4..8].copy_from_slice(&12u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&1u32.to_le_bytes());
    bytes[20..24].copy_from_slice(&(references.len() as u32).to_le_bytes());
    bytes[24..32].copy_from_slice(&40u64.to_le_bytes());
    bytes[32..40].copy_from_slice(&(material_start as u64).to_le_bytes());
    for (index, reference) in references.iter().enumerate() {
        assert!(reference.len() < 256);
        let start = 40 + index * 272;
        bytes[start..start + 4].copy_from_slice(&0x241f_5debu32.to_le_bytes());
        bytes[start + 16..start + 16 + reference.len()].copy_from_slice(reference.as_bytes());
        bytes[start + 16 + reference.len()] = 0;
    }
    bytes
}

pub struct Materials(pub BTreeMap<PackageFileId, Vec<u8>>);

impl ReplacementAssetContentReader for Materials {
    fn read_asset_content(
        &self,
        id: &PackageFileId,
        limit: u64,
    ) -> ReplacementAdapterResult<Vec<u8>> {
        let bytes = self
            .0
            .get(id)
            .ok_or(ReplacementAdapterError::SourceContentUnavailable)?;
        assert!((bytes.len() as u64) <= limit);
        Ok(bytes.clone())
    }
}

pub fn request(paths: &[&str], source: &str, target: &str, carrier: bool) -> RetargetPlanRequest {
    let assets = paths
        .iter()
        .map(|path| ReplacementAsset::new(PackageFileId::new(*path), *path))
        .collect::<Vec<_>>();
    let analysis = MhwReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets.clone(),
        })
        .unwrap();
    let source = analysis
        .sources()
        .iter()
        .find(|value| value.internal_id() == source)
        .unwrap();
    let catalog = MhwReplacementCatalog.replacement_catalog().unwrap();
    let target = catalog
        .targets()
        .iter()
        .find(|value| {
            value.internal_id() == target
                && value.target_type() == source.source_type()
                && value
                    .metadata()
                    .get("path_family")
                    .and_then(serde_json::Value::as_str)
                    == Some(source.path_family())
        })
        .unwrap();
    RetargetPlanRequest {
        game_id: GameId::mhw(),
        binding: ReplacementBinding::new(
            ReplacementBindingId::parse(format!("binding-{}", source.internal_id())).unwrap(),
            ModId::new("synthetic"),
            ProfileId::new("default"),
            source.id().clone(),
            target.id().clone(),
            0,
        )
        .unwrap(),
        assets,
        carries_package_companions: carrier,
    }
}

pub fn references(bytes: &[u8]) -> Vec<String> {
    let count = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
    (0..count)
        .map(|index| {
            let field = &bytes[56 + index * 272..312 + index * 272];
            let end = field.iter().position(|byte| *byte == 0).unwrap();
            String::from_utf8(field[..end].to_vec()).unwrap()
        })
        .collect()
}
