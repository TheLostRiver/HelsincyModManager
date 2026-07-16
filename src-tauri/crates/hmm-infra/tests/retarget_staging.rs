use anyhow::Result;
use hmm_core::{
    FileLayer, InstallFileProvider, InstallPlan, InstallTargetPath, ModId, PackageFileId,
};
use hmm_infra::{
    FileSystemInstallSourceFileReader, FileSystemRetargetStagingMaterializer,
    RetargetStagingInstallSourceFileReader,
};
use hmm_ports::{
    InstallSourceFileReader, RetargetStagingError, RetargetStagingFile, RetargetStagingMaterializer,
};
use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;

fn target(path: &str) -> InstallTargetPath {
    InstallTargetPath::parse(path, ["nativePC"]).expect("target path")
}

fn file(package_file_id: &str, target_path: &str) -> RetargetStagingFile {
    RetargetStagingFile::new(PackageFileId::new(package_file_id), target(target_path))
}

#[test]
fn staging_copies_source_bytes_to_final_relative_target_and_mapped_reader_preserves_identity() {
    let temp = tempfile::tempdir().expect("temp root");
    let source_root = temp.path().join("source");
    let source_path = source_root.join("package").join("body.bin");
    fs::create_dir_all(source_path.parent().expect("source parent")).expect("source parent");
    fs::write(&source_path, b"retarget-bytes").expect("source bytes");
    let staging_root = temp.path().join("staging");
    let source_reader = Arc::new(FileSystemInstallSourceFileReader::new(source_root));
    let materializer =
        FileSystemRetargetStagingMaterializer::new(staging_root.clone(), source_reader);
    let staged_file = file(
        "package/body.bin",
        "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
    );

    materializer
        .materialize(std::slice::from_ref(&staged_file))
        .expect("materialize");

    assert_eq!(
        fs::read(&source_path).expect("source remains"),
        b"retarget-bytes"
    );
    assert_eq!(
        fs::read(staging_root.join(staged_file.target_path().as_str())).expect("staged bytes"),
        b"retarget-bytes"
    );

    let plan = InstallPlan::from_providers([InstallFileProvider::new(
        ModId::new("mod-a"),
        staged_file.package_file_id().clone(),
        staged_file.target_path().clone(),
        FileLayer::new("base", 0),
    )]);
    let reader = RetargetStagingInstallSourceFileReader::from_install_plan(staging_root, &plan)
        .expect("mapped reader");
    assert_eq!(
        reader
            .read_source_file(staged_file.package_file_id())
            .expect("read by original package id"),
        b"retarget-bytes"
    );
}

#[test]
fn staging_rejects_case_insensitive_target_collision_before_writing() {
    let temp = tempfile::tempdir().expect("temp root");
    let source_root = temp.path().join("source");
    fs::create_dir_all(&source_root).expect("source root");
    fs::write(source_root.join("first.bin"), b"first").expect("first source");
    fs::write(source_root.join("second.bin"), b"second").expect("second source");
    let staging_root = temp.path().join("staging");
    let materializer = FileSystemRetargetStagingMaterializer::new(
        staging_root.clone(),
        Arc::new(FileSystemInstallSourceFileReader::new(source_root)),
    );

    let error = materializer
        .materialize(&[
            file("first.bin", "nativePC/Armor/Body.bin"),
            file("second.bin", "nativePC/armor/body.bin"),
        ])
        .expect_err("case-insensitive collision");

    assert_eq!(error, RetargetStagingError::CaseInsensitiveTargetCollision);
    assert!(!staging_root.exists());
}

struct SelectiveReader {
    files: BTreeMap<PackageFileId, Vec<u8>>,
    fail: PackageFileId,
}

impl InstallSourceFileReader for SelectiveReader {
    fn read_source_file(&self, package_file_id: &PackageFileId) -> Result<Vec<u8>> {
        if package_file_id == &self.fail {
            anyhow::bail!("injected source failure");
        }
        self.files
            .get(package_file_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing source"))
    }
}

#[test]
fn staging_source_failure_removes_all_partial_output() {
    let temp = tempfile::tempdir().expect("temp root");
    let staging_root = temp.path().join("staging");
    let pending_root = temp.path().join(".staging.partial");
    let first_id = PackageFileId::new("first.bin");
    let second_id = PackageFileId::new("second.bin");
    let materializer = FileSystemRetargetStagingMaterializer::new(
        staging_root.clone(),
        Arc::new(SelectiveReader {
            files: BTreeMap::from([(first_id.clone(), b"first".to_vec())]),
            fail: second_id.clone(),
        }),
    );

    let error = materializer
        .materialize(&[
            RetargetStagingFile::new(first_id, target("nativePC/first.bin")),
            RetargetStagingFile::new(second_id, target("nativePC/second.bin")),
        ])
        .expect_err("second source fails");

    assert_eq!(error, RetargetStagingError::SourceUnavailable);
    assert!(!staging_root.exists());
    assert!(!pending_root.exists());
}
