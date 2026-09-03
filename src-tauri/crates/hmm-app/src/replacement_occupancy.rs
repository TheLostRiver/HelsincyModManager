use crate::install_manifest_query::InstallManifestQueryService;
use hmm_core::{ModId, ProfileId, ReplacementTargetId};
use hmm_ports::ModImportResultRepository;
use std::sync::Arc;

/// 跨 Mod 同目标占用的展示投影：谁占用了哪个目标，以及占用方叫什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementTargetOccupancy {
    pub target_id: ReplacementTargetId,
    pub mod_id: ModId,
    pub display_name: String,
}

/// 把安装清单里的占用事实映射成前端可直接展示的占用方名单。
///
/// 这里只做展示投影，**不承担门禁职责**。跨 Mod 同目标的硬门禁在预览、任务期
/// 计划构建和 commit 三层（计划构建时合成阻断冲突，见 `append_cross_mod_target_conflicts`），
/// 不依赖本查询。因此清单不可信、读取失败或展示名无法解析时一律 fail-open：
/// 宁可少给一条提示，也不能用不可信事实误导玩家去卸载无辜的 Mod。
pub struct ReplacementOccupancyService {
    manifest_query: Arc<InstallManifestQueryService>,
    results: Arc<dyn ModImportResultRepository>,
}

impl ReplacementOccupancyService {
    pub fn new(
        manifest_query: Arc<InstallManifestQueryService>,
        results: Arc<dyn ModImportResultRepository>,
    ) -> Self {
        Self {
            manifest_query,
            results,
        }
    }

    /// 列出该 profile 下**其他 Mod** 已占用的替换目标。
    ///
    /// 自身占用不算占用：玩家重选自己已装的目标走的是 target switch，不属于跨 Mod 冲突。
    pub fn list_occupancy(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Vec<ReplacementTargetOccupancy> {
        let Ok(occupied) = self
            .manifest_query
            .query_replacement_target_occupancy(profile_id, mod_id)
        else {
            return Vec::new();
        };

        occupied
            .into_iter()
            .map(|entry| {
                // 展示名只是提示，解析失败时退回稳定 Mod id：占用事实本身来自
                // 可信清单，不能因为名字解析失败就丢掉这条占用（丢掉会让前端
                // 漏掉禁用，玩家撞到硬门禁时只剩一句无 actionable 的报错）。
                let display_name = self
                    .display_name(&entry.mod_id)
                    .unwrap_or_else(|| entry.mod_id.as_str().to_owned());
                ReplacementTargetOccupancy {
                    target_id: entry.target_id,
                    mod_id: entry.mod_id,
                    display_name,
                }
            })
            .collect()
    }

    fn display_name(&self, mod_id: &ModId) -> Option<String> {
        let logical = self.results.get_mod(mod_id).ok()??;
        let revision = self
            .results
            .get_revision(&logical.display_revision_id)
            .ok()??;
        Some(revision.display_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install_manifest_query::InstallManifestQueryService;
    use anyhow::anyhow;
    use hmm_core::{
        FileLayer, InstallManifest, InstallManifestEntry,
        InstallManifestStatus as CoreManifestStatus, InstallTargetPath, ModRevisionId,
        PackageFileId, PreviewImageRejectionReason, ReplacementBinding, ReplacementBindingId,
        ReplacementBindingSnapshot, ReplacementSourceId, ReplacementTargetKind,
    };
    use hmm_ports::{
        ModImportResultRepository, StoredImportPreviewImage, StoredLogicalMod,
        StoredModImportAnalysis, StoredModOriginProvenance, StoredModPackageMetadata,
        StoredModRevision,
    };
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    #[derive(Clone)]
    struct FakeInstallManifestRepository {
        manifest: Option<InstallManifest>,
        fail: bool,
    }

    impl hmm_ports::InstallManifestRepository for FakeInstallManifestRepository {
        fn load_manifest(
            &self,
            _profile_id: &ProfileId,
        ) -> anyhow::Result<Option<InstallManifest>> {
            if self.fail {
                return Err(anyhow!("manifest storage is unavailable"));
            }
            Ok(self.manifest.clone())
        }

        fn save_manifest(&self, _manifest: &InstallManifest) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeResultRepository {
        revisions: Mutex<BTreeMap<String, String>>,
        fail: bool,
    }

    impl FakeResultRepository {
        /// `mod_id -> display_name` 的极简映射，缺省即视为解析失败。
        fn with_names(names: &[(&str, &str)]) -> Self {
            Self {
                revisions: Mutex::new(
                    names
                        .iter()
                        .map(|(mod_id, name)| ((*mod_id).to_owned(), (*name).to_owned()))
                        .collect(),
                ),
                fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                revisions: Mutex::new(BTreeMap::new()),
                fail: true,
            }
        }
    }

    /// revision id 里带上 mod id，让 get_revision 能反查到正确的展示名。
    fn revision_id_for(mod_id: &str) -> ModRevisionId {
        ModRevisionId::new(format!("revision-{mod_id}"))
    }

    impl ModImportResultRepository for FakeResultRepository {
        fn get_mod(&self, mod_id: &ModId) -> anyhow::Result<Option<StoredLogicalMod>> {
            if self.fail {
                return Err(anyhow!("import catalog is unavailable"));
            }
            let names = self.revisions.lock().expect("names lock");
            if !names.contains_key(mod_id.as_str()) {
                return Ok(None);
            }
            Ok(Some(StoredLogicalMod {
                mod_id: mod_id.clone(),
                origin_revision_id: revision_id_for(mod_id.as_str()),
                display_revision_id: revision_id_for(mod_id.as_str()),
                origin_provenance: StoredModOriginProvenance::Imported,
            }))
        }

        fn get_revision(
            &self,
            revision_id: &ModRevisionId,
        ) -> anyhow::Result<Option<StoredModRevision>> {
            if self.fail {
                return Err(anyhow!("import catalog is unavailable"));
            }
            let names = self.revisions.lock().expect("names lock");
            let Some(mod_id) = revision_id.as_str().strip_prefix("revision-") else {
                return Ok(None);
            };
            let Some(display_name) = names.get(mod_id).cloned() else {
                return Ok(None);
            };
            Ok(Some(StoredModRevision {
                revision_id: revision_id.clone(),
                mod_id: ModId::new(mod_id),
                import_task_id: "task-v1".to_owned(),
                package_id: "package-v1".to_owned(),
                display_name,
                metadata: StoredModPackageMetadata::default(),
                preview_image: StoredImportPreviewImage::Fallback {
                    reason: PreviewImageRejectionReason::Missing,
                },
            }))
        }

        fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> anyhow::Result<()> {
            Ok(())
        }

        fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
            Ok(Vec::new())
        }

        fn get_analysis(&self, _mod_id: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
            Ok(None)
        }
    }

    fn service(
        manifest: Option<InstallManifest>,
        names: &[(&str, &str)],
    ) -> ReplacementOccupancyService {
        ReplacementOccupancyService::new(
            Arc::new(InstallManifestQueryService::new(Arc::new(
                FakeInstallManifestRepository {
                    manifest,
                    fail: false,
                },
            ))),
            Arc::new(FakeResultRepository::with_names(names)),
        )
    }

    fn manifest_entry(mod_id: &str, target_path: &str) -> InstallManifestEntry {
        InstallManifestEntry {
            target_path: InstallTargetPath::parse(target_path, ["nativePC"]).expect("target path"),
            mod_id: ModId::new(mod_id),
            revision_id: None,
            package_file_id: PackageFileId::new(target_path),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: None,
            adopted: false,
        }
    }

    fn binding(mod_id: &str, target_id: &str) -> ReplacementBindingSnapshot {
        ReplacementBindingSnapshot::new(
            ReplacementBinding::new(
                // binding id 必须 per-mod 唯一：同一 profile 下重复 id 会让清单校验失败。
                ReplacementBindingId::parse(format!("binding-{mod_id}")).expect("binding id"),
                ModId::new(mod_id),
                ProfileId::new("default"),
                ReplacementSourceId::parse("mhw:weapon:wp:one001").expect("source id"),
                ReplacementTargetId::parse(target_id).expect("target id"),
                1,
            )
            .expect("binding"),
            None,
            "one001",
            target_id,
            "wp/one001",
            "wp/one002",
            ReplacementTargetKind::parse("weapon").expect("target kind"),
        )
        .expect("replacement snapshot")
    }

    fn manifest_with(
        entries: Vec<InstallManifestEntry>,
        bindings: Vec<ReplacementBindingSnapshot>,
    ) -> InstallManifest {
        let mut manifest = InstallManifest::completed(ProfileId::new("default"), entries);
        manifest.replacement_bindings = bindings;
        manifest
    }

    #[test]
    fn occupancy_lists_other_mods_with_display_names() {
        let manifest = manifest_with(
            vec![
                manifest_entry("mod-a", "nativePC/wp/one001/one001.mod3"),
                manifest_entry("mod-b", "nativePC/wp/one002/one002.mod3"),
            ],
            vec![
                binding("mod-a", "mhw:weapon:one001"),
                binding("mod-b", "mhw:weapon:one002"),
            ],
        );
        let service = service(Some(manifest), &[("mod-b", "Weapon Mod B")]);

        let occupancy = service.list_occupancy(&ProfileId::new("default"), &ModId::new("mod-a"));

        assert_eq!(
            occupancy,
            vec![ReplacementTargetOccupancy {
                target_id: ReplacementTargetId::parse("mhw:weapon:one002").expect("target id"),
                mod_id: ModId::new("mod-b"),
                display_name: "Weapon Mod B".to_owned(),
            }]
        );
    }

    #[test]
    fn occupancy_ignores_the_queried_mod_own_binding() {
        let manifest = manifest_with(
            vec![manifest_entry("mod-a", "nativePC/wp/one001/one001.mod3")],
            vec![binding("mod-a", "mhw:weapon:one001")],
        );
        let service = service(Some(manifest), &[("mod-a", "Weapon Mod A")]);

        // 自身占用走 target switch，不是跨 Mod 冲突，不该弹占用提示。
        assert!(service
            .list_occupancy(&ProfileId::new("default"), &ModId::new("mod-a"))
            .is_empty());
    }

    #[test]
    fn occupancy_falls_back_to_mod_id_when_display_name_is_unresolvable() {
        let manifest = manifest_with(
            vec![manifest_entry("mod-b", "nativePC/wp/one002/one002.mod3")],
            vec![binding("mod-b", "mhw:weapon:one002")],
        );
        let service = service(Some(manifest), &[]);

        let occupancy = service.list_occupancy(&ProfileId::new("default"), &ModId::new("mod-a"));

        // 占用事实来自可信清单，名字解析失败也不能丢掉这条占用。
        assert_eq!(occupancy.len(), 1);
        assert_eq!(occupancy[0].display_name, "mod-b");
    }

    #[test]
    fn occupancy_is_empty_when_catalog_read_fails() {
        let manifest = manifest_with(
            vec![manifest_entry("mod-b", "nativePC/wp/one002/one002.mod3")],
            vec![binding("mod-b", "mhw:weapon:one002")],
        );
        let service = ReplacementOccupancyService::new(
            Arc::new(InstallManifestQueryService::new(Arc::new(
                FakeInstallManifestRepository {
                    manifest: Some(manifest),
                    fail: false,
                },
            ))),
            Arc::new(FakeResultRepository::failing()),
        );

        let occupancy = service.list_occupancy(&ProfileId::new("default"), &ModId::new("mod-a"));

        assert_eq!(occupancy.len(), 1);
        assert_eq!(occupancy[0].display_name, "mod-b");
    }

    #[test]
    fn occupancy_is_empty_when_manifest_is_unavailable() {
        let service = ReplacementOccupancyService::new(
            Arc::new(InstallManifestQueryService::new(Arc::new(
                FakeInstallManifestRepository {
                    manifest: None,
                    fail: true,
                },
            ))),
            Arc::new(FakeResultRepository::with_names(&[(
                "mod-b",
                "Weapon Mod B",
            )])),
        );

        assert!(service
            .list_occupancy(&ProfileId::new("default"), &ModId::new("mod-a"))
            .is_empty());
    }

    #[test]
    fn occupancy_is_empty_when_manifest_status_is_not_trusted() {
        let mut manifest = manifest_with(
            vec![manifest_entry("mod-b", "nativePC/wp/one002/one002.mod3")],
            vec![binding("mod-b", "mhw:weapon:one002")],
        );
        // InFlight 清单的条目随时会被回滚，不能作为"已占用"的依据。
        manifest.status = CoreManifestStatus::Committing;
        let service = service(Some(manifest), &[("mod-b", "Weapon Mod B")]);

        assert!(service
            .list_occupancy(&ProfileId::new("default"), &ModId::new("mod-a"))
            .is_empty());
    }

    #[test]
    fn occupancy_skips_bindings_without_installed_entries() {
        // 只有 binding 没有清单条目：该 Mod 并未真正占用目标文件。
        let manifest = manifest_with(Vec::new(), vec![binding("mod-b", "mhw:weapon:one002")]);
        let service = service(Some(manifest), &[("mod-b", "Weapon Mod B")]);

        assert!(service
            .list_occupancy(&ProfileId::new("default"), &ModId::new("mod-a"))
            .is_empty());
    }

    #[test]
    fn occupancy_keeps_one_entry_per_target() {
        let manifest = manifest_with(
            vec![
                manifest_entry("mod-b", "nativePC/wp/one002/one002.mod3"),
                manifest_entry("mod-c", "nativePC/wp/one002/one002.mrl3"),
            ],
            vec![
                binding("mod-b", "mhw:weapon:one002"),
                binding("mod-c", "mhw:weapon:one002"),
            ],
        );
        let service = service(Some(manifest), &[("mod-b", "Weapon Mod B")]);

        let occupancy = service.list_occupancy(&ProfileId::new("default"), &ModId::new("mod-a"));

        assert_eq!(occupancy.len(), 1);
        assert_eq!(occupancy[0].mod_id.as_str(), "mod-b");
    }
}
