use hmm_core::{ContentTransformInvocation, InstallTargetPath, PackageFileId};
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentTransformOutput {
    bytes: Vec<u8>,
    canonical_mapping_sha256: String,
}

impl ContentTransformOutput {
    pub fn new(bytes: Vec<u8>, canonical_mapping_sha256: impl Into<String>) -> Self {
        Self {
            bytes,
            canonical_mapping_sha256: canonical_mapping_sha256.into(),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn canonical_mapping_sha256(&self) -> &str {
        &self.canonical_mapping_sha256
    }
}

pub struct ContentTransformRequest<'a> {
    invocation: &'a ContentTransformInvocation,
    package_file_id: &'a PackageFileId,
    source_bytes: &'a [u8],
    dependencies: &'a BTreeMap<PackageFileId, Vec<u8>>,
}

impl<'a> ContentTransformRequest<'a> {
    pub fn new(
        invocation: &'a ContentTransformInvocation,
        package_file_id: &'a PackageFileId,
        source_bytes: &'a [u8],
        dependencies: &'a BTreeMap<PackageFileId, Vec<u8>>,
    ) -> Self {
        Self {
            invocation,
            package_file_id,
            source_bytes,
            dependencies,
        }
    }

    pub fn invocation(&self) -> &ContentTransformInvocation {
        self.invocation
    }

    pub fn package_file_id(&self) -> &PackageFileId {
        self.package_file_id
    }

    pub fn source_bytes(&self) -> &[u8] {
        self.source_bytes
    }

    pub fn dependencies(&self) -> &BTreeMap<PackageFileId, Vec<u8>> {
        self.dependencies
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContentTransformerError {
    #[error("content transform invocation is invalid")]
    InvalidInvocation,
    #[error("content transform dependency is unavailable")]
    DependencyUnavailable,
    #[error("content transformer rejected the input: {code}")]
    Rejected { code: String },
}

impl ContentTransformerError {
    pub fn rejected(code: impl Into<String>) -> Self {
        let code = code.into();
        let safe = !code.is_empty()
            && code.len() <= 128
            && code.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            });
        if safe {
            Self::Rejected { code }
        } else {
            Self::InvalidInvocation
        }
    }

    pub fn code(&self) -> &str {
        match self {
            Self::InvalidInvocation => "content_transform_invocation_invalid",
            Self::DependencyUnavailable => "content_transform_dependency_unavailable",
            Self::Rejected { code } => code,
        }
    }
}

pub trait ContentTransformer: Send + Sync {
    fn transformer_id(&self) -> &'static str;

    fn transformer_version(&self) -> u32;

    fn transform(
        &self,
        request: ContentTransformRequest<'_>,
    ) -> Result<ContentTransformOutput, ContentTransformerError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContentTransformerRegistryError {
    #[error("content transformer registration identity is invalid")]
    InvalidRegistration,
    #[error("content transformer identity is registered more than once")]
    DuplicateRegistration,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContentTransformDispatchError {
    #[error("content transformer is unavailable")]
    TransformerUnavailable,
    #[error(transparent)]
    TransformFailed(#[from] ContentTransformerError),
}

pub struct ContentTransformerRegistry {
    transformers: BTreeMap<(String, u32), Arc<dyn ContentTransformer>>,
}

impl ContentTransformerRegistry {
    pub fn new(
        transformers: Vec<Arc<dyn ContentTransformer>>,
    ) -> Result<Self, ContentTransformerRegistryError> {
        let mut registered = BTreeMap::new();
        for transformer in transformers {
            let id = transformer.transformer_id();
            let version = transformer.transformer_version();
            let identity_valid = !id.is_empty()
                && id.len() <= 128
                && version > 0
                && id.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'_' | b'-')
                });
            if !identity_valid {
                return Err(ContentTransformerRegistryError::InvalidRegistration);
            }
            if registered
                .insert((id.to_owned(), version), transformer)
                .is_some()
            {
                return Err(ContentTransformerRegistryError::DuplicateRegistration);
            }
        }
        Ok(Self {
            transformers: registered,
        })
    }

    pub fn empty() -> Self {
        Self {
            transformers: BTreeMap::new(),
        }
    }

    pub fn transform(
        &self,
        request: ContentTransformRequest<'_>,
    ) -> Result<ContentTransformOutput, ContentTransformDispatchError> {
        let identity = (
            request.invocation().transformer_id().to_owned(),
            request.invocation().transformer_version(),
        );
        let transformer = self
            .transformers
            .get(&identity)
            .ok_or(ContentTransformDispatchError::TransformerUnavailable)?;
        transformer.transform(request).map_err(Into::into)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetargetStagingFile {
    package_file_id: PackageFileId,
    target_path: InstallTargetPath,
    content_transform: Option<ContentTransformInvocation>,
}

impl RetargetStagingFile {
    pub fn new(package_file_id: PackageFileId, target_path: InstallTargetPath) -> Self {
        Self {
            package_file_id,
            target_path,
            content_transform: None,
        }
    }

    pub fn with_content_transform(mut self, invocation: ContentTransformInvocation) -> Self {
        self.content_transform = Some(invocation);
        self
    }

    pub fn package_file_id(&self) -> &PackageFileId {
        &self.package_file_id
    }

    pub fn target_path(&self) -> &InstallTargetPath {
        &self.target_path
    }

    pub fn content_transform(&self) -> Option<&ContentTransformInvocation> {
        self.content_transform.as_ref()
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RetargetStagingError {
    #[error("retarget staging batch cannot be empty")]
    EmptyBatch,
    #[error("retarget staging batch contains a duplicate package file")]
    DuplicatePackageFile,
    #[error("retarget staging targets collide on a case-insensitive filesystem")]
    CaseInsensitiveTargetCollision,
    #[error("retarget staging source is unavailable")]
    SourceUnavailable,
    #[error("retarget staging destination is unavailable")]
    DestinationUnavailable,
    #[error("retarget staging target is unsafe")]
    UnsafeTarget,
    #[error("retarget staging write failed")]
    WriteFailed,
    #[error("retarget staging publish failed")]
    PublishFailed,
    #[error("retarget staging cleanup failed")]
    CleanupFailed,
    #[error("retarget staging source digest changed")]
    SourceDigestMismatch,
    #[error("retarget staging transformer is unavailable")]
    TransformerUnavailable,
    #[error("retarget staging transform failed: {code}")]
    TransformFailed { code: String },
    #[error("retarget staging transform output is invalid")]
    TransformOutputInvalid,
}

pub trait RetargetStagingMaterializer: Send + Sync {
    fn materialize(&self, files: &[RetargetStagingFile]) -> Result<(), RetargetStagingError>;
}
