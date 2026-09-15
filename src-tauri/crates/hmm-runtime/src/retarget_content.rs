use hmm_app::ReinstallCandidateSourceReader;
use hmm_core::{
    ContentTransformInvocation, InstalledFileSummary, ModRevisionId, PackageFileId, RetargetPlan,
};
use hmm_games_mhw::{MhwEquipmentMrl3TexturePathTransformer, MhwWeaponMrl3TexturePathTransformer};
use hmm_ports::{ContentTransformRequest, ContentTransformerRegistry, StoredModRevision};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

pub(crate) fn registry() -> anyhow::Result<ContentTransformerRegistry> {
    Ok(ContentTransformerRegistry::new(vec![
        Arc::new(MhwWeaponMrl3TexturePathTransformer),
        Arc::new(MhwEquipmentMrl3TexturePathTransformer),
    ])?)
}

pub(crate) fn invocations(
    plans: &[RetargetPlan],
) -> BTreeMap<PackageFileId, ContentTransformInvocation> {
    plans
        .iter()
        .flat_map(|plan| plan.actions())
        .filter_map(|action| {
            action
                .content_transform()
                .map(|invocation| (action.package_file_id().clone(), invocation.clone()))
        })
        .collect()
}

/// 只读预览使用与 staging 相同的变换器，不在磁盘上生成候选文件。
pub(crate) struct RetargetCandidateReader {
    original: Arc<dyn ReinstallCandidateSourceReader>,
    revision: ModRevisionId,
    transforms: BTreeMap<PackageFileId, ContentTransformInvocation>,
    registry: ContentTransformerRegistry,
    inputs: Mutex<BTreeMap<PackageFileId, InstalledFileSummary>>,
}

impl RetargetCandidateReader {
    pub(crate) fn new(
        original: Arc<dyn ReinstallCandidateSourceReader>,
        revision: ModRevisionId,
        plans: &[RetargetPlan],
    ) -> anyhow::Result<Self> {
        let mut transforms = BTreeMap::new();
        for action in plans.iter().flat_map(|plan| plan.actions()) {
            if let Some(invocation) = action.content_transform() {
                anyhow::ensure!(
                    transforms
                        .insert(action.package_file_id().clone(), invocation.clone())
                        .is_none(),
                    "duplicate transform source"
                );
            }
        }
        Ok(Self {
            original,
            revision,
            transforms,
            registry: registry()?,
            inputs: Mutex::new(BTreeMap::new()),
        })
    }

    pub(crate) fn input_summaries(
        &self,
    ) -> anyhow::Result<BTreeMap<PackageFileId, InstalledFileSummary>> {
        self.inputs
            .lock()
            .map(|inputs| inputs.clone())
            .map_err(|_| anyhow::anyhow!("source summaries unavailable"))
    }

    fn read_original(
        &self,
        candidate: &StoredModRevision,
        id: &PackageFileId,
    ) -> anyhow::Result<Vec<u8>> {
        anyhow::ensure!(
            candidate.revision_id == self.revision,
            "candidate revision changed"
        );
        let bytes = self.original.read_candidate_source_file(candidate, id)?;
        let summary = InstalledFileSummary {
            size_bytes: bytes.len() as u64,
            sha256: digest(&bytes),
        };
        let mut inputs = self
            .inputs
            .lock()
            .map_err(|_| anyhow::anyhow!("source summaries unavailable"))?;
        if let Some(previous) = inputs.insert(id.clone(), summary.clone()) {
            anyhow::ensure!(
                previous == summary,
                "candidate source changed during preview"
            );
        }
        Ok(bytes)
    }
}

impl ReinstallCandidateSourceReader for RetargetCandidateReader {
    fn read_candidate_source_file(
        &self,
        candidate: &StoredModRevision,
        id: &PackageFileId,
    ) -> anyhow::Result<Vec<u8>> {
        let bytes = self.read_original(candidate, id)?;
        let Some(invocation) = self.transforms.get(id) else {
            return Ok(bytes);
        };
        anyhow::ensure!(
            digest(&bytes) == invocation.source_content_sha256(),
            "transform source changed"
        );
        let mut dependencies = BTreeMap::new();
        for (id, hash) in invocation.dependencies() {
            let content = self.read_original(candidate, id)?;
            anyhow::ensure!(&digest(&content) == hash, "transform dependency changed");
            dependencies.insert(id.clone(), content);
        }
        let output = self.registry.transform(ContentTransformRequest::new(
            invocation,
            id,
            &bytes,
            &dependencies,
        ))?;
        anyhow::ensure!(
            digest(output.bytes()) == invocation.output_content_sha256()
                && output.canonical_mapping_sha256() == invocation.canonical_mapping_sha256(),
            "transform output changed"
        );
        Ok(output.into_bytes())
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
