use hmm_core::{ContentTransformInvocation, PackageFileId};
use hmm_ports::{
    ContentTransformDispatchError, ContentTransformOutput, ContentTransformRequest,
    ContentTransformer, ContentTransformerError, ContentTransformerRegistry,
    ContentTransformerRegistryError,
};
use std::collections::BTreeMap;
use std::sync::Arc;

struct EchoTransformer;

impl ContentTransformer for EchoTransformer {
    fn transformer_id(&self) -> &'static str {
        "test.echo.v1"
    }

    fn transformer_version(&self) -> u32 {
        1
    }

    fn transform(
        &self,
        request: ContentTransformRequest<'_>,
    ) -> Result<ContentTransformOutput, ContentTransformerError> {
        Ok(ContentTransformOutput::new(
            request.source_bytes().to_vec(),
            request.invocation().canonical_mapping_sha256().to_owned(),
        ))
    }
}

fn invocation(id: &str, version: u32) -> ContentTransformInvocation {
    ContentTransformInvocation::new(
        1,
        id,
        version,
        "a".repeat(64),
        "a".repeat(64),
        "b".repeat(64),
        BTreeMap::new(),
        BTreeMap::new(),
    )
    .expect("invocation")
}

#[test]
fn registry_rejects_duplicate_identity() {
    let result = ContentTransformerRegistry::new(vec![
        Arc::new(EchoTransformer) as Arc<dyn ContentTransformer>,
        Arc::new(EchoTransformer),
    ]);

    assert!(matches!(
        result,
        Err(ContentTransformerRegistryError::DuplicateRegistration)
    ));
}

#[test]
fn registry_dispatches_only_exact_id_and_version() {
    let registry = ContentTransformerRegistry::new(vec![
        Arc::new(EchoTransformer) as Arc<dyn ContentTransformer>
    ])
    .expect("registry");
    let package_file_id = PackageFileId::new("source.bin");
    let dependencies = BTreeMap::new();
    let valid = invocation("test.echo.v1", 1);
    let output = registry
        .transform(ContentTransformRequest::new(
            &valid,
            &package_file_id,
            b"source",
            &dependencies,
        ))
        .expect("transform");
    assert_eq!(output.bytes(), b"source");

    let stale = invocation("test.echo.v1", 2);
    assert!(matches!(
        registry.transform(ContentTransformRequest::new(
            &stale,
            &package_file_id,
            b"source",
            &dependencies,
        )),
        Err(ContentTransformDispatchError::TransformerUnavailable)
    ));
}
