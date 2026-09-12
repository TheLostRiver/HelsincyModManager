use hmm_core::{installed_file_summary, InstalledFileSummary, PackageFileId};
use hmm_ports::InstallSourceFileReader;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// 记录实际交给暂存／转换器的原始字节，提交复核不把转换后的摘要误当作原包摘要。
pub(super) struct RecordingRetargetSourceReader {
    source: Arc<dyn InstallSourceFileReader>,
    summaries: Mutex<BTreeMap<PackageFileId, InstalledFileSummary>>,
}

impl RecordingRetargetSourceReader {
    pub(super) fn new(source: Arc<dyn InstallSourceFileReader>) -> Self {
        Self {
            source,
            summaries: Mutex::new(BTreeMap::new()),
        }
    }

    pub(super) fn summaries(
        &self,
    ) -> anyhow::Result<BTreeMap<PackageFileId, InstalledFileSummary>> {
        self.summaries
            .lock()
            .map(|summaries| summaries.clone())
            .map_err(|_| anyhow::anyhow!("source recording unavailable"))
    }
}

impl InstallSourceFileReader for RecordingRetargetSourceReader {
    fn read_source_file(&self, id: &PackageFileId) -> anyhow::Result<Vec<u8>> {
        let bytes = self.source.read_source_file(id)?;
        let summary = installed_file_summary(&bytes);
        let previous = self
            .summaries
            .lock()
            .map_err(|_| anyhow::anyhow!("source recording unavailable"))?
            .insert(id.clone(), summary.clone());
        anyhow::ensure!(
            previous.is_none_or(|previous| previous == summary),
            "source changed while staging"
        );
        Ok(bytes)
    }
}
