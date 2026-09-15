use super::material_table::{field_text, texture_fields, PATH_CAPACITY};
use hmm_core::{
    ContentTransformInvocation, InstallTargetPath, CONTENT_TRANSFORM_INVOCATION_SCHEMA_VERSION,
};
use hmm_ports::{
    ContentTransformOutput, ContentTransformRequest, ContentTransformer, ContentTransformerError,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const TRANSFORMER_ID: &str = "mhw.equipment.mrl3-texture-path.v1";
const TRANSFORMER_VERSION: u32 = 1;

pub struct MhwEquipmentMrl3TexturePathTransformer;

impl ContentTransformer for MhwEquipmentMrl3TexturePathTransformer {
    fn transformer_id(&self) -> &'static str {
        TRANSFORMER_ID
    }
    fn transformer_version(&self) -> u32 {
        TRANSFORMER_VERSION
    }

    fn transform(
        &self,
        request: ContentTransformRequest<'_>,
    ) -> Result<ContentTransformOutput, ContentTransformerError> {
        if !request.dependencies().is_empty() {
            return Err(ContentTransformerError::InvalidInvocation);
        }
        let parameters = request.invocation().parameters();
        let output = apply_updates(request.source_bytes(), parameters)
            .map_err(ContentTransformerError::rejected)?;
        if sha256(request.source_bytes()) != request.invocation().source_content_sha256()
            || sha256(&output) != request.invocation().output_content_sha256()
            || mapping_digest(parameters) != request.invocation().canonical_mapping_sha256()
        {
            return Err(ContentTransformerError::InvalidInvocation);
        }
        Ok(ContentTransformOutput::new(
            output,
            mapping_digest(parameters),
        ))
    }
}

pub(super) fn invocation(
    bytes: &[u8],
    destinations: &BTreeMap<String, InstallTargetPath>,
) -> Result<Option<ContentTransformInvocation>, &'static str> {
    let fields = texture_fields(bytes)?;
    let mut updates = BTreeMap::new();
    for (index, field) in fields.iter().enumerate() {
        let original =
            field_text(&bytes[field.clone()]).ok_or("equipment_material_reference_unsafe")?;
        if original.is_empty() {
            continue;
        }
        let reference =
            TextureReference::parse(original).ok_or("equipment_material_reference_unsafe")?;
        let Some(destination) = destinations.get(&reference.key) else {
            continue;
        };
        let target = reference.render(destination);
        if target != original {
            updates.insert(format!("texture_{index}"), target);
        }
    }
    if updates.is_empty() {
        return Ok(None);
    }
    let output = apply_updates(bytes, &updates)?;
    ContentTransformInvocation::new(
        CONTENT_TRANSFORM_INVOCATION_SCHEMA_VERSION,
        TRANSFORMER_ID,
        TRANSFORMER_VERSION,
        sha256(bytes),
        sha256(&output),
        mapping_digest(&updates),
        BTreeMap::new(),
        updates,
    )
    .map(Some)
    .map_err(|_| "equipment_material_transform_invalid")
}

fn apply_updates(
    bytes: &[u8],
    updates: &BTreeMap<String, String>,
) -> Result<Vec<u8>, &'static str> {
    let fields = texture_fields(bytes)?;
    if updates.is_empty() || updates.len() > fields.len() {
        return Err("equipment_material_transform_invalid");
    }
    let mut output = bytes.to_vec();
    for (key, value) in updates {
        let index = key
            .strip_prefix("texture_")
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|index| key == &format!("texture_{index}"))
            .ok_or("equipment_material_transform_invalid")?;
        let field = fields
            .get(index)
            .ok_or("equipment_material_transform_invalid")?;
        if value.len() >= PATH_CAPACITY {
            return Err("equipment_material_path_too_long");
        }
        if TextureReference::parse(value).is_none() {
            return Err("equipment_material_reference_unsafe");
        }
        let target = &mut output[field.clone()];
        target.fill(0);
        target[..value.len()].copy_from_slice(value.as_bytes());
    }
    // 只写已验证的 256 字节字段，其余头、纹理属性、材质和作者附加数据逐字节保留。
    Ok(output)
}

struct TextureReference {
    key: String,
    root_prefix: bool,
    extension: bool,
    separator: char,
}

impl TextureReference {
    fn parse(value: &str) -> Option<Self> {
        let normalized = value.replace('\\', "/");
        // 在加 nativePC 前验证引用本身，不能把绝对路径伪装成 root 下的相对段。
        if normalized
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".." || part.contains(':'))
        {
            return None;
        }
        let root_prefix = normalized
            .split('/')
            .next()?
            .eq_ignore_ascii_case("nativePC");
        let relative = if root_prefix {
            normalized.split_once('/')?.1
        } else {
            &normalized
        };
        let extension = relative.to_ascii_lowercase().ends_with(".tex");
        let path = format!("nativePC/{relative}{}", if extension { "" } else { ".tex" });
        let path = InstallTargetPath::parse(path, ["nativePC"]).ok()?;
        Some(Self {
            key: path.windows_key(),
            root_prefix,
            extension,
            separator: if value.contains('\\') && !value.contains('/') {
                '\\'
            } else {
                '/'
            },
        })
    }

    fn render(&self, destination: &InstallTargetPath) -> String {
        let path = destination.as_str().trim_end_matches(['.', ' ']);
        let path = if self.root_prefix {
            path
        } else {
            path.strip_prefix("nativePC/").expect("validated game root")
        };
        let path = if self.extension {
            path
        } else {
            &path[..path.len() - 4]
        };
        path.replace('/', &self.separator.to_string())
    }
}

fn mapping_digest(updates: &BTreeMap<String, String>) -> String {
    let mut hash = Sha256::new();
    hash.update(TRANSFORMER_ID.as_bytes());
    for (key, value) in updates {
        for part in [key, value] {
            hash.update((part.len() as u64).to_le_bytes());
            hash.update(part.as_bytes());
        }
    }
    format!("{:x}", hash.finalize())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
