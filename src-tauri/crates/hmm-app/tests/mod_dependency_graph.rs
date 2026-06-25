use hmm_app::ModDependencyGraphService;
use hmm_ports::{
    ModImportResultRepository, StoredImportPreviewImage, StoredModImportAnalysis,
    StoredModPackageMetadata,
};
use std::sync::Mutex;

struct FakeModImportResultRepository {
    records: Mutex<Vec<StoredModImportAnalysis>>,
}

impl FakeModImportResultRepository {
    fn new(records: Vec<StoredModImportAnalysis>) -> Self {
        Self {
            records: Mutex::new(records),
        }
    }
}

impl ModImportResultRepository for FakeModImportResultRepository {
    fn save_analysis(&self, analysis: &StoredModImportAnalysis) -> anyhow::Result<()> {
        self.records
            .lock()
            .expect("records lock")
            .push(analysis.clone());
        Ok(())
    }

    fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
        Ok(self.records.lock().expect("records lock").clone())
    }

    fn get_analysis(&self, mod_id: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
        Ok(self
            .records
            .lock()
            .expect("records lock")
            .iter()
            .find(|record| record.mod_id == mod_id)
            .cloned())
    }
}

#[test]
fn dependency_graph_reports_declared_edges_without_claiming_install_status() {
    let service = ModDependencyGraphService::new(std::sync::Arc::new(
        FakeModImportResultRepository::new(vec![
            stored_record(
                "armor-pack",
                "Armor Pack",
                vec![
                    " stracker-loader ".to_owned(),
                    "missing-core".to_owned(),
                    "stracker-loader".to_owned(),
                    " ".to_owned(),
                ],
            ),
            stored_record("stracker-loader", "Stracker Loader", Vec::new()),
        ]),
    ));

    let graph = service
        .get_mod_dependency_graph()
        .expect("dependency graph can be built");

    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.nodes[0].mod_id, "armor-pack");
    assert_eq!(graph.nodes[0].name, "Armor Pack");
    assert_eq!(graph.nodes[1].mod_id, "stracker-loader");
    assert_eq!(graph.edges.len(), 2);
    assert_eq!(graph.edges[0].source_mod_id, "armor-pack");
    assert_eq!(graph.edges[0].dependency, "stracker-loader");
    assert_eq!(
        graph.edges[0].matched_imported_mod_id.as_deref(),
        Some("stracker-loader")
    );
    assert_eq!(graph.edges[1].dependency, "missing-core");
    assert!(graph.edges[1].matched_imported_mod_id.is_none());
}

fn stored_record(
    mod_id: &str,
    display_name: &str,
    dependencies: Vec<String>,
) -> StoredModImportAnalysis {
    StoredModImportAnalysis {
        mod_id: mod_id.to_owned(),
        task_id: format!("task-{mod_id}"),
        package_id: mod_id.to_owned(),
        display_name: display_name.to_owned(),
        metadata: StoredModPackageMetadata {
            dependencies,
            ..StoredModPackageMetadata::default()
        },
        preview_image: StoredImportPreviewImage::Fallback {
            reason: hmm_core::PreviewImageRejectionReason::Missing,
        },
    }
}
