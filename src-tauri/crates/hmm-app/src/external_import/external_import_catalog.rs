use super::ExternalImportBatchError;
use crate::mod_import::stored_revision_from_result;
use crate::ModImportAnalysisResult;
use hmm_core::{
    ExternalImportBatch, ExternalImportBatchId, ExternalImportCandidate, ExternalImportProvenance,
    ExternalImportSelectionDecision, ModId,
};
use hmm_ports::{
    ModImportCatalogUpsert, ModImportResultRepository, StoredLogicalMod, StoredModOriginProvenance,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct CatalogIndex {
    pub(super) by_content_fingerprint: BTreeMap<String, CatalogExternalImport>,
    pub(super) display_names: BTreeSet<String>,
}

pub(super) struct CatalogExternalImport {
    pub(super) mod_id: ModId,
    batch_id: ExternalImportBatchId,
    source_item_key_hash: String,
}

impl CatalogExternalImport {
    pub(super) fn matches_candidate(
        &self,
        batch: &ExternalImportBatch,
        candidate: &ExternalImportCandidate,
    ) -> bool {
        self.batch_id == batch.batch_id
            && self.source_item_key_hash == candidate.source_item_key_hash
    }
}

impl CatalogIndex {
    pub(super) fn load(catalog: &dyn ModImportResultRepository) -> anyhow::Result<Self> {
        let mut by_content_fingerprint = BTreeMap::new();
        let mut display_names = BTreeSet::new();
        for logical_mod in catalog.list_mods()? {
            if let Some(revision) = catalog.get_revision(&logical_mod.display_revision_id)? {
                display_names.insert(normalize_display_name(&revision.display_name));
            }
            if let StoredModOriginProvenance::ExternalImport { provenance } =
                &logical_mod.origin_provenance
            {
                by_content_fingerprint.insert(
                    provenance.content_fingerprint.clone(),
                    CatalogExternalImport {
                        mod_id: logical_mod.mod_id.clone(),
                        batch_id: provenance.batch_id.clone(),
                        source_item_key_hash: provenance.source_item_key_hash.clone(),
                    },
                );
            }
        }
        Ok(Self {
            by_content_fingerprint,
            display_names,
        })
    }

    pub(super) fn record(&mut self, logical_mod: &StoredLogicalMod, display_name: &str) {
        self.display_names
            .insert(normalize_display_name(display_name));
        if let StoredModOriginProvenance::ExternalImport { provenance } =
            &logical_mod.origin_provenance
        {
            self.by_content_fingerprint.insert(
                provenance.content_fingerprint.clone(),
                CatalogExternalImport {
                    mod_id: logical_mod.mod_id.clone(),
                    batch_id: provenance.batch_id.clone(),
                    source_item_key_hash: provenance.source_item_key_hash.clone(),
                },
            );
        }
    }
}

pub(super) fn normalize_display_name(value: &str) -> String {
    value.trim().to_lowercase()
}

pub(super) struct PendingCatalogImport {
    pub(super) candidate: ExternalImportCandidate,
    pub(super) decision: Option<ExternalImportSelectionDecision>,
    pub(super) analysis: ModImportAnalysisResult,
    pub(super) content_fingerprint: String,
    pub(super) upsert: ModImportCatalogUpsert,
}

impl PendingCatalogImport {
    pub(super) fn new(
        batch: &ExternalImportBatch,
        candidate: &ExternalImportCandidate,
        decision: Option<ExternalImportSelectionDecision>,
        analysis: ModImportAnalysisResult,
        imported_at_unix_millis: u64,
    ) -> Result<Self, ExternalImportBatchError> {
        let provenance = ExternalImportProvenance {
            adapter_id: batch.adapter_id.clone(),
            batch_id: batch.batch_id.clone(),
            source_item_key_hash: candidate.source_item_key_hash.clone(),
            content_fingerprint: candidate.content_fingerprint.clone(),
            imported_at_unix_millis,
        };
        provenance
            .validate()
            .map_err(|_| ExternalImportBatchError::CatalogUnavailable)?;
        let mod_id = ModId::new(&analysis.package_id);
        let revision = stored_revision_from_result(&mod_id, &analysis);
        let logical_mod = StoredLogicalMod {
            mod_id,
            origin_revision_id: revision.revision_id.clone(),
            display_revision_id: revision.revision_id.clone(),
            origin_provenance: StoredModOriginProvenance::ExternalImport { provenance },
        };
        Ok(Self {
            candidate: candidate.clone(),
            decision,
            content_fingerprint: candidate.content_fingerprint.clone(),
            analysis,
            upsert: ModImportCatalogUpsert {
                logical_mod,
                revision,
            },
        })
    }
}

pub(super) fn merge_external_metadata_hint(
    analysis: &mut ModImportAnalysisResult,
    candidate: &ExternalImportCandidate,
) {
    let metadata = &candidate.metadata_hint;
    if analysis.metadata.display_name.is_none() {
        if let Some(display_name) = metadata.display_name.clone() {
            analysis.metadata.display_name = Some(display_name.clone());
            analysis.display_name = display_name;
        }
    }
    if analysis.metadata.author.is_none() {
        analysis.metadata.author = metadata.author.clone();
    }
    if analysis.metadata.version.is_none() {
        analysis.metadata.version = metadata.version.clone();
    }
}
