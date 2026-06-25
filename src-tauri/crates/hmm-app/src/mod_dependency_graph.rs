use hmm_ports::{ModImportResultRepository, StoredModImportAnalysis};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDependencyGraph {
    pub nodes: Vec<ModDependencyGraphNode>,
    pub edges: Vec<ModDependencyGraphEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDependencyGraphNode {
    pub mod_id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDependencyGraphEdge {
    pub source_mod_id: String,
    pub dependency: String,
    pub matched_imported_mod_id: Option<String>,
}

pub struct ModDependencyGraphService {
    result_repository: Arc<dyn ModImportResultRepository>,
}

impl ModDependencyGraphService {
    pub fn new(result_repository: Arc<dyn ModImportResultRepository>) -> Self {
        Self { result_repository }
    }

    pub fn get_mod_dependency_graph(&self) -> anyhow::Result<ModDependencyGraph> {
        let records = self.result_repository.list_analysis()?;
        let imported_mod_ids = imported_mod_ids_by_normalized_key(&records);
        let nodes = records
            .iter()
            .map(|record| ModDependencyGraphNode {
                mod_id: record.mod_id.clone(),
                name: record.display_name.clone(),
            })
            .collect();
        let edges = records
            .iter()
            .flat_map(|record| dependency_edges_for_record(record, &imported_mod_ids))
            .collect();

        Ok(ModDependencyGraph { nodes, edges })
    }
}

fn imported_mod_ids_by_normalized_key(
    records: &[StoredModImportAnalysis],
) -> HashMap<String, String> {
    records
        .iter()
        .map(|record| {
            (
                normalize_dependency_key(&record.mod_id),
                record.mod_id.clone(),
            )
        })
        .collect()
}

fn dependency_edges_for_record(
    record: &StoredModImportAnalysis,
    imported_mod_ids: &HashMap<String, String>,
) -> Vec<ModDependencyGraphEdge> {
    let mut seen = HashSet::new();
    record
        .metadata
        .dependencies
        .iter()
        .filter_map(|dependency| {
            let dependency = dependency.trim();
            if dependency.is_empty() {
                return None;
            }

            let normalized = normalize_dependency_key(dependency);
            if !seen.insert(normalized.clone()) {
                return None;
            }

            Some(ModDependencyGraphEdge {
                source_mod_id: record.mod_id.clone(),
                dependency: dependency.to_owned(),
                matched_imported_mod_id: imported_mod_ids.get(&normalized).cloned(),
            })
        })
        .collect()
}

fn normalize_dependency_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}
