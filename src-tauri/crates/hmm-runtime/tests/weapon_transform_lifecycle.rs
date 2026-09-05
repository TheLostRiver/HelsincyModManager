use hmm_app::{
    CommitInstallPlanRequest, GamePrerequisiteDecision, GamePrerequisiteDecisionProvider,
    GamePrerequisiteDecisionStatus, InstallCommitService, ReinstallCandidatePlanError,
    ReinstallCandidatePlanRequest, ReinstallCandidatePlanner, ReinstallCandidateSourceReader,
    ReinstallCommitService, ReinstallPreparation, ReinstallPreviewRequest, ReinstallPreviewService,
    ReinstallTaskExecutor, ReinstallTaskExecutorService, ReplacementService,
    RetargetMaterializeError, UninstallModRequest, UninstallModService,
};
use hmm_core::{
    ContentTransformerIdentity, FileLayer, GameId, InstallPlan, InstallTargetPath, ModId,
    ModRevisionId, PackageFileId, PreviewImageRejectionReason, ProfileId, ReplacementAdapterFacts,
    ReplacementBinding, ReplacementBindingId, ReplacementSource, ReplacementTargetId,
    ReplacementTargetKind, RetargetAction, RetargetPlan,
};
use hmm_games_mhw::{
    analyze_mhw_weapon_assets, build_mhw_weapon_mrl3_transform_invocation,
    MhwWeaponMrl3TexturePathTransformer, WeaponMainId, WeaponModelPair,
    MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID, MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_VERSION,
};
use hmm_infra::{
    FileSystemInstallBackupStore, FileSystemInstallGameFileSystem,
    FileSystemInstallSourceFileReader, FileSystemRetargetStagingMaterializer,
    JsonInstallManifestRepository, JsonInstallRecoveryRecordRepository,
    JsonModImportResultRepository, JsonReinstallRecoveryTransactionRepository,
    RetargetStagingInstallSourceFileReader,
};
use hmm_ports::{
    ContentTransformer, ContentTransformerRegistry, InstallBackupStore, InstallGameFileSystem,
    InstallManifestRepository, InstallRecoveryRecordRepository, InstallSourceFileReader,
    ModImportResultRepository, ReinstallRecoveryTransactionRepository, ReinstallSnapshotStore,
    ReplacementAsset, StoredImportPreviewImage, StoredLogicalMod, StoredModOriginProvenance,
    StoredModPackageMetadata, StoredModRevision,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

const MOD3_HEADER_SIZE: usize = 320;
const MOD3_MATERIAL_ENTRY_SIZE: usize = 128;
const MOD3_MESH_ENTRY_SIZE: usize = 80;
const MRL3_HEADER_SIZE: usize = 40;
const MRL3_TEXTURE_ENTRY_SIZE: usize = 272;
const MRL3_MATERIAL_ENTRY_SIZE: usize = 56;
const ARTIFICIAL_MATERIAL_HASH: u32 = 0xa7f6_8bf8;
const SOURCE_MOD3_ID: &str = "weapon/model.mod3";
const SOURCE_MRL3_ID: &str = "weapon/model.mrl3";
const SOURCE_MOD3_PATH: &str = "nativePC/wp/one/one001/mod/one001.mod3";
const SOURCE_MRL3_PATH: &str = "nativePC/wp/one/one001/mod/one001.mrl3";

struct ReadyPrerequisites;

impl GamePrerequisiteDecisionProvider for ReadyPrerequisites {
    fn prerequisite_decision(&self, _game_id: &GameId) -> GamePrerequisiteDecision {
        GamePrerequisiteDecision {
            game_id: GameId::mhw(),
            status: GamePrerequisiteDecisionStatus::Ready,
            rules_version: Some(1),
            codes: Vec::new(),
        }
    }
}

struct UnusedCandidatePlanner;

impl ReinstallCandidatePlanner for UnusedCandidatePlanner {
    fn build_candidate_plan(
        &self,
        _request: ReinstallCandidatePlanRequest<'_>,
    ) -> Result<InstallPlan, ReinstallCandidatePlanError> {
        Err(ReinstallCandidatePlanError::Unavailable)
    }
}

struct StagedCandidateSource {
    reader: RetargetStagingInstallSourceFileReader,
}

impl ReinstallCandidateSourceReader for StagedCandidateSource {
    fn read_candidate_source_file(
        &self,
        _candidate: &StoredModRevision,
        package_file_id: &PackageFileId,
    ) -> anyhow::Result<Vec<u8>> {
        self.reader.read_source_file(package_file_id)
    }
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn artificial_mod3() -> Vec<u8> {
    let material_offset = MOD3_HEADER_SIZE;
    let mesh_offset = material_offset + MOD3_MATERIAL_ENTRY_SIZE;
    let vertex_offset = mesh_offset + MOD3_MESH_ENTRY_SIZE + 4;
    let vertex_buffer_size = 36usize;
    let face_offset = vertex_offset + vertex_buffer_size;
    let face_buffer_size = 8usize;
    let vertex_remap_offset = face_offset + face_buffer_size;
    let mut bytes = vec![0u8; vertex_remap_offset + 24];
    write_u32(&mut bytes, 0, 0x0044_4f4d);
    write_u16(&mut bytes, 4, 237);
    write_u16(&mut bytes, 8, 1);
    write_u16(&mut bytes, 10, 1);
    write_u32(&mut bytes, 12, 3);
    write_u32(&mut bytes, 16, 3);
    write_u64(&mut bytes, 24, vertex_buffer_size as u64);
    write_u64(&mut bytes, 64, material_offset as u64);
    write_u64(&mut bytes, 72, mesh_offset as u64);
    write_u64(&mut bytes, 80, vertex_offset as u64);
    write_u64(&mut bytes, 88, face_offset as u64);
    write_u64(&mut bytes, 96, vertex_remap_offset as u64);
    let material = b"ArtificialWeaponMaterial";
    bytes[material_offset..material_offset + material.len()].copy_from_slice(material);
    write_u16(&mut bytes, mesh_offset + 2, 3);
    write_u16(&mut bytes, mesh_offset + 6, 0);
    write_u16(&mut bytes, mesh_offset + 8, 1);
    bytes[mesh_offset + 14] = 12;
    write_u32(&mut bytes, mesh_offset + 32, 3);
    write_u32(&mut bytes, vertex_remap_offset, 4);
    bytes
}

fn artificial_mrl3() -> Vec<u8> {
    let texture_offset = MRL3_HEADER_SIZE;
    let material_offset = texture_offset + MRL3_TEXTURE_ENTRY_SIZE;
    let material_end = material_offset + MRL3_MATERIAL_ENTRY_SIZE;
    let resource_offset = (material_end + 15) & !15;
    let mut bytes = vec![0u8; resource_offset + 16];
    write_u32(&mut bytes, 0, 0x004c_524d);
    write_u32(&mut bytes, 4, 12);
    write_u32(&mut bytes, 16, 1);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, texture_offset as u64);
    write_u64(&mut bytes, 32, material_offset as u64);
    write_u32(&mut bytes, texture_offset, 0x241f_5deb);
    let path = b"wp\\one\\one001\\tex\\weapon_BM";
    bytes[texture_offset + 16..texture_offset + 16 + path.len()].copy_from_slice(path);
    write_u32(&mut bytes, material_offset, 0x4516_e7ab);
    write_u32(&mut bytes, material_offset + 4, ARTIFICIAL_MATERIAL_HASH);
    write_u32(&mut bytes, material_offset + 16, 16);
    write_u16(&mut bytes, material_offset + 22, 2);
    write_u64(&mut bytes, material_offset + 48, resource_offset as u64);
    bytes
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn source_pair() -> WeaponModelPair {
    analyze_mhw_weapon_assets(&[
        ReplacementAsset::new(PackageFileId::new(SOURCE_MOD3_ID), SOURCE_MOD3_PATH),
        ReplacementAsset::new(PackageFileId::new(SOURCE_MRL3_ID), SOURCE_MRL3_PATH),
    ])
    .expect("artificial source closure")
    .sole_unit()
    .expect("恰好一个可重定向单元")
    .pairs()[0]
        .clone()
}

fn weapon_plan(
    mod_id: &ModId,
    profile_id: &ProfileId,
    revision_id: &ModRevisionId,
    target_main: &str,
    binding_time: u128,
    mod3: &[u8],
    mrl3: &[u8],
) -> RetargetPlan {
    let pair = source_pair();
    let target_main_id = WeaponMainId::parse(target_main).expect("target main id");
    let source_id = analyze_mhw_weapon_assets(&[
        ReplacementAsset::new(PackageFileId::new(SOURCE_MOD3_ID), SOURCE_MOD3_PATH),
        ReplacementAsset::new(PackageFileId::new(SOURCE_MRL3_ID), SOURCE_MRL3_PATH),
    ])
    .expect("source closure")
    .sole_unit()
    .expect("恰好一个可重定向单元")
    .source_id()
    .clone();
    let invocation = build_mhw_weapon_mrl3_transform_invocation(&pair, &target_main_id, mod3, mrl3)
        .expect("transform invocation");
    let actions = vec![
        RetargetAction::new(
            PackageFileId::new(SOURCE_MOD3_ID),
            InstallTargetPath::parse(SOURCE_MOD3_PATH, ["nativePC"]).expect("source MOD3"),
            pair.mod3()
                .model_path()
                .retarget(&target_main_id)
                .expect("target MOD3"),
            source_id.clone(),
            "one001",
            target_main,
            "wp/one",
            "wp/one",
        )
        .expect("MOD3 action"),
        RetargetAction::new(
            PackageFileId::new(SOURCE_MRL3_ID),
            InstallTargetPath::parse(SOURCE_MRL3_PATH, ["nativePC"]).expect("source MRL3"),
            pair.mrl3()
                .model_path()
                .retarget(&target_main_id)
                .expect("target MRL3"),
            source_id.clone(),
            "one001",
            target_main,
            "wp/one",
            "wp/one",
        )
        .expect("MRL3 action")
        .with_content_transform(invocation),
    ];
    let source = ReplacementSource::new(
        source_id.clone(),
        GameId::mhw(),
        ReplacementTargetKind::parse("weapon").expect("weapon kind"),
        "one001",
        "wp/one",
        true,
    )
    .expect("replacement source");
    let binding = ReplacementBinding::new(
        ReplacementBindingId::parse("binding-weapon").expect("binding id"),
        mod_id.clone(),
        profile_id.clone(),
        source_id,
        ReplacementTargetId::parse(format!("mhw:weapon:{target_main}")).expect("target id"),
        binding_time,
    )
    .expect("binding");
    let plan = RetargetPlan::new(binding, source, actions, Vec::new()).expect("retarget plan");
    let facts = ReplacementAdapterFacts::new(
        1,
        "mhw.weapon",
        "mrl3-texture-path",
        1,
        sha256(&[mod3, mrl3].concat()),
        sha256(b"main"),
        plan.content_transform_set_sha256(),
    )
    .expect("adapter facts")
    .with_transformers(
        vec![ContentTransformerIdentity::new(
            MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID,
            MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_VERSION,
        )
        .expect("transformer identity")],
        1,
        plan.actions().len() as u32,
    )
    .expect("transformer facts");
    let plan = plan.with_adapter_facts(facts).expect("sealed plan");
    assert_eq!(revision_id.as_str(), "revision-weapon-v1");
    plan
}

fn registry() -> Arc<ContentTransformerRegistry> {
    Arc::new(
        ContentTransformerRegistry::new(vec![
            Arc::new(MhwWeaponMrl3TexturePathTransformer) as Arc<dyn ContentTransformer>
        ])
        .expect("transformer registry"),
    )
}

fn materialize(
    source_root: &Path,
    staging_root: &Path,
    plan: RetargetPlan,
    revision_id: &ModRevisionId,
) -> Result<InstallPlan, RetargetMaterializeError> {
    let materializer = FileSystemRetargetStagingMaterializer::new_with_registry(
        staging_root.to_path_buf(),
        Arc::new(FileSystemInstallSourceFileReader::new(
            source_root.to_path_buf(),
        )),
        registry(),
    );
    ReplacementService::new(Vec::new())
        .materialize_retarget(
            &materializer,
            hmm_app::MaterializeRetargetRequest {
                plan,
                layer: FileLayer::new("base", 0),
                revision_id: Some(revision_id.clone()),
            },
        )
        .map(|materialized| materialized.into_parts().1)
}

fn snapshot_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, current: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        if !current.exists() {
            return;
        }
        for entry in fs::read_dir(current).expect("read temp tree") {
            let entry = entry.expect("tree entry");
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("relative temp path")
                    .to_string_lossy()
                    .replace('\\', "/");
                files.insert(relative, fs::read(path).expect("read temp file"));
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

#[test]
fn artificial_weapon_install_switch_restart_and_uninstall_restore_exact_baseline() {
    let temp = tempfile::tempdir().expect("temp root");
    let app_data = temp.path().join("app-data");
    let game_root = temp.path().join("game");
    let source_root = temp.path().join("source");
    fs::create_dir_all(source_root.join("weapon")).expect("source root");
    let mod3 = artificial_mod3();
    let mrl3 = artificial_mrl3();
    fs::write(source_root.join(SOURCE_MOD3_ID), &mod3).expect("source MOD3");
    fs::write(source_root.join(SOURCE_MRL3_ID), &mrl3).expect("source MRL3");

    let switch_mod3 = game_root.join("nativePC/wp/one/one003/mod/one003.mod3");
    let switch_mrl3 = game_root.join("nativePC/wp/one/one003/mod/one003.mrl3");
    fs::create_dir_all(switch_mod3.parent().expect("switch parent")).expect("game root");
    fs::write(&switch_mod3, b"baseline-mod3").expect("baseline MOD3");
    fs::write(&switch_mrl3, b"baseline-mrl3").expect("baseline MRL3");
    let baseline = snapshot_tree(&game_root);

    let mod_id = ModId::new("mod-weapon");
    let profile_id = ProfileId::new("default");
    let revision_id = ModRevisionId::new("revision-weapon-v1");
    let catalog_path = app_data.join("mod-import/results.json");
    let concrete_catalog = JsonModImportResultRepository::new(catalog_path);
    concrete_catalog
        .save_new_mod(
            &StoredLogicalMod {
                mod_id: mod_id.clone(),
                origin_revision_id: revision_id.clone(),
                display_revision_id: revision_id.clone(),
                origin_provenance: StoredModOriginProvenance::Imported,
            },
            &StoredModRevision {
                revision_id: revision_id.clone(),
                mod_id: mod_id.clone(),
                import_task_id: "artificial-import".to_owned(),
                package_id: revision_id.as_str().to_owned(),
                display_name: "Artificial Weapon Fixture".to_owned(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: StoredImportPreviewImage::Fallback {
                    reason: PreviewImageRejectionReason::Missing,
                },
            },
        )
        .expect("persist artificial revision");
    let catalog: Arc<dyn ModImportResultRepository> = Arc::new(concrete_catalog);
    let game: Arc<dyn InstallGameFileSystem> =
        Arc::new(FileSystemInstallGameFileSystem::new(game_root.clone()));
    let backups: Arc<dyn InstallBackupStore> = Arc::new(FileSystemInstallBackupStore::new(
        app_data.join("install/backups"),
    ));
    let manifests: Arc<dyn InstallManifestRepository> = Arc::new(
        JsonInstallManifestRepository::new(app_data.join("install/manifests")),
    );
    let install_recovery: Arc<dyn InstallRecoveryRecordRepository> = Arc::new(
        JsonInstallRecoveryRecordRepository::new(app_data.join("install/recovery")),
    );
    let reinstall_recovery: Arc<dyn ReinstallRecoveryTransactionRepository> =
        Arc::new(JsonReinstallRecoveryTransactionRepository::new(
            app_data.join("install/reinstall-recovery"),
        ));

    let initial_staging = temp.path().join("initial-staging");
    let initial_plan = materialize(
        &source_root,
        &initial_staging,
        weapon_plan(
            &mod_id,
            &profile_id,
            &revision_id,
            "one002",
            1,
            &mod3,
            &mrl3,
        ),
        &revision_id,
    )
    .expect("materialize initial target");
    let initial_source: Arc<dyn InstallSourceFileReader> = Arc::new(
        RetargetStagingInstallSourceFileReader::from_install_plan(
            initial_staging.clone(),
            &initial_plan,
        )
        .expect("initial staging reader"),
    );
    let installed = InstallCommitService::new_with_recovery_records(
        initial_source,
        Arc::clone(&game),
        Arc::clone(&backups),
        Arc::clone(&manifests),
        Arc::clone(&install_recovery),
    )
    .commit_plan_for_revision(
        CommitInstallPlanRequest {
            game_id: GameId::mhw(),
            profile_id: profile_id.clone(),
            plan: initial_plan,
        },
        mod_id.clone(),
        revision_id.clone(),
    )
    .expect("initial install");
    assert_eq!(installed.manifest.entries.len(), 2);
    assert!(installed.manifest.replacement_bindings[0]
        .adapter_facts()
        .is_some());
    let installed_facts = installed.manifest.replacement_bindings[0]
        .adapter_facts()
        .expect("installed adapter facts");
    assert_eq!(installed_facts.part_count(), 1);
    assert_eq!(installed_facts.file_count(), 2);
    assert_eq!(
        installed_facts.transformer_identities()[0].transformer_id(),
        MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID
    );
    assert!(install_recovery
        .list_records(&profile_id)
        .expect("initial recovery records")
        .is_empty());
    fs::remove_dir_all(&initial_staging).expect("drop initial staging");

    let restarted_manifest_repository: Arc<dyn InstallManifestRepository> = Arc::new(
        JsonInstallManifestRepository::new(app_data.join("install/manifests")),
    );
    let restarted_manifest = restarted_manifest_repository
        .load_manifest(&profile_id)
        .expect("restart manifest read")
        .expect("installed manifest");
    assert_eq!(restarted_manifest, installed.manifest);

    let switch_staging = temp.path().join("switch-staging");
    let switch_plan = materialize(
        &source_root,
        &switch_staging,
        weapon_plan(
            &mod_id,
            &profile_id,
            &revision_id,
            "one003",
            1,
            &mod3,
            &mrl3,
        ),
        &revision_id,
    )
    .expect("materialize switch target");
    let switch_source = Arc::new(StagedCandidateSource {
        reader: RetargetStagingInstallSourceFileReader::from_install_plan(
            switch_staging.clone(),
            &switch_plan,
        )
        .expect("switch staging reader"),
    });
    let preview = Arc::new(ReinstallPreviewService::new(
        Arc::new(ReadyPrerequisites),
        Arc::clone(&catalog),
        Arc::new(UnusedCandidatePlanner),
        switch_source.clone(),
        Arc::clone(&game),
        Arc::clone(&backups),
        Arc::clone(&restarted_manifest_repository),
        Arc::clone(&reinstall_recovery),
    ));
    let preparation = preview
        .prepare_replacement_target_switch(
            ReinstallPreviewRequest {
                game_id: GameId::mhw(),
                profile_id: profile_id.clone(),
                mod_id: mod_id.clone(),
                candidate_revision_id: revision_id.clone(),
                layer: FileLayer::new("base", 0),
            },
            switch_plan,
        )
        .expect("switch preview");
    let prepared = match preparation {
        ReinstallPreparation::Ready(prepared) => prepared,
        ReinstallPreparation::Blocked(preview) => {
            panic!("artificial target switch must be ready: {preview:?}")
        }
    };
    let plan_token = prepared.plan_token().to_owned();
    let snapshots: Arc<dyn ReinstallSnapshotStore> = Arc::new(FileSystemInstallBackupStore::new(
        app_data.join("install/backups"),
    ));
    let executor = ReinstallTaskExecutorService::new(
        preview,
        Arc::new(ReinstallCommitService::new(
            Arc::clone(&catalog),
            switch_source,
            Arc::clone(&game),
            Arc::clone(&backups),
            Arc::clone(&restarted_manifest_repository),
            Arc::clone(&reinstall_recovery),
            snapshots,
        )),
    );
    executor.revalidate(&prepared).expect("switch revalidation");
    let switched = executor
        .commit(*prepared, &plan_token)
        .expect("true reinstall switch");
    assert_eq!(switched.manifest.entries.len(), 2);
    assert_eq!(
        switched.manifest.replacement_bindings[0].target_internal_id(),
        "one003"
    );
    assert!(switched.manifest.replacement_bindings[0]
        .adapter_facts()
        .is_some());
    assert!(!game_root
        .join("nativePC/wp/one/one002/mod/one002.mod3")
        .exists());
    assert!(!game_root
        .join("nativePC/wp/one/one002/mod/one002.mrl3")
        .exists());
    assert_ne!(
        fs::read(&switch_mod3).expect("switched MOD3"),
        b"baseline-mod3"
    );
    assert_ne!(
        fs::read(&switch_mrl3).expect("switched MRL3"),
        b"baseline-mrl3"
    );
    assert!(reinstall_recovery
        .list_transactions(&profile_id)
        .expect("switch recovery transactions")
        .is_empty());
    fs::remove_dir_all(&switch_staging).expect("drop switch staging");

    let final_manifest_repository: Arc<dyn InstallManifestRepository> = Arc::new(
        JsonInstallManifestRepository::new(app_data.join("install/manifests")),
    );
    let restarted_switched = final_manifest_repository
        .load_manifest(&profile_id)
        .expect("switched restart read")
        .expect("switched manifest");
    assert_eq!(restarted_switched, switched.manifest);
    assert_eq!(
        restarted_switched.replacement_bindings[0].adapter_facts(),
        switched.manifest.replacement_bindings[0].adapter_facts()
    );

    let uninstalled = UninstallModService::new(
        Arc::clone(&game),
        Arc::clone(&backups),
        Arc::clone(&final_manifest_repository),
    )
    .uninstall_mod_for_revision(
        UninstallModRequest {
            game_id: GameId::mhw(),
            profile_id: profile_id.clone(),
            mod_id: mod_id.clone(),
        },
        revision_id,
    )
    .expect("manifest uninstall");
    assert_eq!(uninstalled.removed_file_count, 0);
    assert_eq!(uninstalled.restored_file_count, 2);
    assert!(uninstalled.manifest.entries.is_empty());
    assert!(uninstalled.manifest.replacement_bindings.is_empty());
    assert_eq!(snapshot_tree(&game_root), baseline);
    assert!(install_recovery
        .list_records(&profile_id)
        .expect("final install recovery records")
        .is_empty());
    assert!(reinstall_recovery
        .list_transactions(&profile_id)
        .expect("final reinstall recovery records")
        .is_empty());
}
