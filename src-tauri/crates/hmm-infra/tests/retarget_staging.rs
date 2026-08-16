use anyhow::Result;
use hmm_core::{
    ContentTransformInvocation, FileLayer, InstallFileProvider, InstallPlan, InstallTargetPath,
    ModId, PackageFileId,
};
use hmm_infra::{
    FileSystemInstallSourceFileReader, FileSystemRetargetStagingMaterializer,
    RetargetStagingInstallSourceFileReader,
};
use hmm_ports::{
    ContentTransformOutput, ContentTransformRequest, ContentTransformer, ContentTransformerError,
    ContentTransformerRegistry, InstallSourceFileReader, RetargetStagingError, RetargetStagingFile,
    RetargetStagingMaterializer,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;

const MAPPING_SHA256: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

struct JoinTransformer;

impl ContentTransformer for JoinTransformer {
    fn transformer_id(&self) -> &'static str {
        "test.join.v1"
    }

    fn transformer_version(&self) -> u32 {
        1
    }

    fn transform(
        &self,
        request: ContentTransformRequest<'_>,
    ) -> Result<ContentTransformOutput, ContentTransformerError> {
        if request.invocation().parameters().get("reject").is_some() {
            return Err(ContentTransformerError::rejected("test_transform_rejected"));
        }
        let dependency = request
            .dependencies()
            .get(&PackageFileId::new("companion.bin"))
            .ok_or(ContentTransformerError::DependencyUnavailable)?;
        let mut output = request.source_bytes().to_vec();
        output.extend_from_slice(dependency);
        Ok(ContentTransformOutput::new(output, MAPPING_SHA256))
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn transform_invocation(
    source: &[u8],
    dependency: &[u8],
    expected_output: &[u8],
    transformer_id: &str,
    parameters: BTreeMap<String, String>,
) -> ContentTransformInvocation {
    ContentTransformInvocation::new(
        1,
        transformer_id,
        1,
        sha256(source),
        sha256(expected_output),
        MAPPING_SHA256,
        BTreeMap::from([(PackageFileId::new("companion.bin"), sha256(dependency))]),
        parameters,
    )
    .expect("invocation")
}

fn transform_registry() -> Arc<ContentTransformerRegistry> {
    Arc::new(
        ContentTransformerRegistry::new(vec![
            Arc::new(JoinTransformer) as Arc<dyn ContentTransformer>
        ])
        .expect("registry"),
    )
}

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

#[test]
fn staging_verifies_dependencies_and_publishes_only_validated_transform_output() {
    let temp = tempfile::tempdir().expect("temp root");
    let source_root = temp.path().join("source");
    fs::create_dir_all(&source_root).expect("source root");
    let source = b"source";
    let dependency = b"-pair";
    let output = b"source-pair";
    fs::write(source_root.join("primary.bin"), source).expect("primary");
    fs::write(source_root.join("companion.bin"), dependency).expect("companion");
    let staging_root = temp.path().join("staging");
    let materializer = FileSystemRetargetStagingMaterializer::new_with_registry(
        staging_root.clone(),
        Arc::new(FileSystemInstallSourceFileReader::new(source_root)),
        transform_registry(),
    );
    let staged = file("primary.bin", "nativePC/transformed.bin").with_content_transform(
        transform_invocation(source, dependency, output, "test.join.v1", BTreeMap::new()),
    );

    materializer
        .materialize(&[staged])
        .expect("validated transform");

    assert_eq!(
        fs::read(staging_root.join("nativePC/transformed.bin")).expect("staged output"),
        output
    );
    assert!(!temp.path().join(".staging.partial").exists());
}

#[test]
fn staging_rejects_source_or_output_digest_drift_without_publishing() {
    for drift in ["source", "output"] {
        let temp = tempfile::tempdir().expect("temp root");
        let source_root = temp.path().join("source");
        fs::create_dir_all(&source_root).expect("source root");
        let source = b"source";
        let dependency = b"-pair";
        fs::write(
            source_root.join("primary.bin"),
            if drift == "source" {
                b"changed".as_slice()
            } else {
                source
            },
        )
        .expect("primary");
        fs::write(source_root.join("companion.bin"), dependency).expect("companion");
        let staging_root = temp.path().join("staging");
        let materializer = FileSystemRetargetStagingMaterializer::new_with_registry(
            staging_root.clone(),
            Arc::new(FileSystemInstallSourceFileReader::new(source_root)),
            transform_registry(),
        );
        let expected_output = if drift == "output" {
            b"wrong-output".as_slice()
        } else {
            b"source-pair".as_slice()
        };
        let staged = file("primary.bin", "nativePC/transformed.bin").with_content_transform(
            transform_invocation(
                source,
                dependency,
                expected_output,
                "test.join.v1",
                BTreeMap::new(),
            ),
        );

        let error = materializer
            .materialize(&[staged])
            .expect_err("digest drift");

        assert_eq!(
            error,
            if drift == "source" {
                RetargetStagingError::SourceDigestMismatch
            } else {
                RetargetStagingError::TransformOutputInvalid
            }
        );
        assert!(!staging_root.exists());
        assert!(!temp.path().join(".staging.partial").exists());
    }
}

#[test]
fn staging_rejects_unknown_or_failed_transform_without_publishing() {
    for failure in ["unknown", "rejected"] {
        let temp = tempfile::tempdir().expect("temp root");
        let source_root = temp.path().join("source");
        fs::create_dir_all(&source_root).expect("source root");
        let source = b"source";
        let dependency = b"-pair";
        fs::write(source_root.join("primary.bin"), source).expect("primary");
        fs::write(source_root.join("companion.bin"), dependency).expect("companion");
        let staging_root = temp.path().join("staging");
        let materializer = FileSystemRetargetStagingMaterializer::new_with_registry(
            staging_root.clone(),
            Arc::new(FileSystemInstallSourceFileReader::new(source_root)),
            transform_registry(),
        );
        let parameters = if failure == "rejected" {
            BTreeMap::from([("reject".to_owned(), "true".to_owned())])
        } else {
            BTreeMap::new()
        };
        let transformer_id = if failure == "unknown" {
            "test.unknown.v1"
        } else {
            "test.join.v1"
        };
        let staged = file("primary.bin", "nativePC/transformed.bin").with_content_transform(
            transform_invocation(
                source,
                dependency,
                b"source-pair",
                transformer_id,
                parameters,
            ),
        );

        let error = materializer
            .materialize(&[staged])
            .expect_err("transform failure");

        match failure {
            "unknown" => assert_eq!(error, RetargetStagingError::TransformerUnavailable),
            _ => assert_eq!(
                error,
                RetargetStagingError::TransformFailed {
                    code: "test_transform_rejected".to_owned()
                }
            ),
        }
        assert!(!staging_root.exists());
        assert!(!temp.path().join(".staging.partial").exists());
    }
}
