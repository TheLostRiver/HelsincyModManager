use super::binary::{parse_model_pair, sha256_hex, ParsedTextureReference};
use super::{WeaponBinaryError, WeaponMainId, WeaponModelPair, WeaponResourceRoot};
use sha2::{Digest, Sha256};
use std::fmt;
use std::ops::Range;

pub const MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID: &str = "mhw.weapon.mrl3-texture-path.v1";
pub const MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_VERSION: u32 = 1;

const MRL3_TEXTURE_PATH_CAPACITY: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponMrl3TransformReport {
    transformer_id: &'static str,
    transformer_version: u32,
    source_sha256: String,
    output_sha256: String,
    mapping_sha256: String,
    texture_reference_count: u32,
    rewritten_reference_count: u32,
    changed_range_count: u32,
    changed_byte_count: u64,
}

impl WeaponMrl3TransformReport {
    pub fn transformer_id(&self) -> &'static str {
        self.transformer_id
    }

    pub fn transformer_version(&self) -> u32 {
        self.transformer_version
    }

    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    pub fn output_sha256(&self) -> &str {
        &self.output_sha256
    }

    pub fn mapping_sha256(&self) -> &str {
        &self.mapping_sha256
    }

    pub fn texture_reference_count(&self) -> u32 {
        self.texture_reference_count
    }

    pub fn rewritten_reference_count(&self) -> u32 {
        self.rewritten_reference_count
    }

    pub fn changed_range_count(&self) -> u32 {
        self.changed_range_count
    }

    pub fn changed_byte_count(&self) -> u64 {
        self.changed_byte_count
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct WeaponMrl3TransformOutput {
    bytes: Vec<u8>,
    report: WeaponMrl3TransformReport,
}

impl fmt::Debug for WeaponMrl3TransformOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WeaponMrl3TransformOutput")
            .field("byte_len", &self.bytes.len())
            .field("report", &self.report)
            .finish()
    }
}

impl WeaponMrl3TransformOutput {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn report(&self) -> &WeaponMrl3TransformReport {
        &self.report
    }
}

pub fn transform_mhw_weapon_mrl3_texture_paths(
    pair: &WeaponModelPair,
    target_main_id: &WeaponMainId,
    mod3_bytes: &[u8],
    mrl3_bytes: &[u8],
) -> Result<WeaponMrl3TransformOutput, WeaponBinaryError> {
    let source_root = pair.mrl3().model_path().root();
    if target_main_id.family() != source_root.family() {
        return Err(WeaponBinaryError::CrossFamilyTarget);
    }

    let (_, parsed_mrl3) = parse_model_pair(pair, mod3_bytes, mrl3_bytes)?;
    let mut output = mrl3_bytes.to_vec();
    let mut expected_references = Vec::with_capacity(parsed_mrl3.textures.len());
    let mut allowed_ranges = Vec::new();
    let mut rewritten_reference_count = 0u32;

    for texture in &parsed_mrl3.textures {
        let (expected, rewritten) = expected_reference(texture, source_root, target_main_id)?;
        if let Some(target) = &rewritten {
            if target.len() >= MRL3_TEXTURE_PATH_CAPACITY {
                return Err(WeaponBinaryError::PathTooLong);
            }
            let field = output
                .get_mut(texture.field_range.clone())
                .ok_or(WeaponBinaryError::OutputInvalid)?;
            field.fill(0);
            field[..target.len()].copy_from_slice(target.as_bytes());
            allowed_ranges.push(texture.field_range.clone());
            rewritten_reference_count = rewritten_reference_count
                .checked_add(1)
                .ok_or(WeaponBinaryError::OutputInvalid)?;
        }
        expected_references.push(expected);
    }

    let changed_ranges = allowed_ranges
        .iter()
        .filter(|range| mrl3_bytes[(*range).clone()] != output[(*range).clone()])
        .cloned()
        .collect::<Vec<_>>();
    let changed_byte_count = mrl3_bytes
        .iter()
        .zip(&output)
        .filter(|(before, after)| before != after)
        .count();
    verify_changed_ranges(mrl3_bytes, &output, &allowed_ranges)?;

    let (_, output_mrl3) = parse_model_pair(pair, mod3_bytes, &output)
        .map_err(|_| WeaponBinaryError::OutputInvalid)?;
    if output_mrl3.material_hashes != parsed_mrl3.material_hashes
        || output_mrl3.textures.len() != expected_references.len()
    {
        return Err(WeaponBinaryError::OutputInvalid);
    }
    for (texture, expected) in output_mrl3.textures.iter().zip(&expected_references) {
        let actual = texture
            .reference
            .as_ref()
            .map(|reference| reference.original());
        if actual != expected.as_deref() {
            return Err(WeaponBinaryError::OutputInvalid);
        }
    }

    let texture_reference_count = parsed_mrl3
        .textures
        .iter()
        .filter(|texture| texture.reference.is_some())
        .count();
    Ok(WeaponMrl3TransformOutput {
        report: WeaponMrl3TransformReport {
            transformer_id: MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID,
            transformer_version: MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_VERSION,
            source_sha256: parsed_mrl3.report.file_sha256().to_owned(),
            output_sha256: sha256_hex(&output),
            mapping_sha256: mapping_sha256(pair, source_root, target_main_id)?,
            texture_reference_count: u32::try_from(texture_reference_count)
                .map_err(|_| WeaponBinaryError::OutputInvalid)?,
            rewritten_reference_count,
            changed_range_count: u32::try_from(changed_ranges.len())
                .map_err(|_| WeaponBinaryError::OutputInvalid)?,
            changed_byte_count: u64::try_from(changed_byte_count)
                .map_err(|_| WeaponBinaryError::OutputInvalid)?,
        },
        bytes: output,
    })
}

fn expected_reference(
    texture: &ParsedTextureReference,
    source_root: &WeaponResourceRoot,
    target_main_id: &WeaponMainId,
) -> Result<(Option<String>, Option<String>), WeaponBinaryError> {
    let Some(reference) = &texture.reference else {
        return Ok((None, None));
    };
    match reference.retarget(source_root, target_main_id)? {
        Some(target) => Ok((Some(target.clone()), Some(target))),
        None => Ok((Some(reference.original().to_owned()), None)),
    }
}

fn verify_changed_ranges(
    source: &[u8],
    output: &[u8],
    allowed_ranges: &[Range<usize>],
) -> Result<(), WeaponBinaryError> {
    if source.len() != output.len() {
        return Err(WeaponBinaryError::OutputInvalid);
    }
    for (index, (before, after)) in source.iter().zip(output).enumerate() {
        if before != after && !allowed_ranges.iter().any(|range| range.contains(&index)) {
            return Err(WeaponBinaryError::OutputInvalid);
        }
    }
    Ok(())
}

fn mapping_sha256(
    pair: &WeaponModelPair,
    source_root: &WeaponResourceRoot,
    target_main_id: &WeaponMainId,
) -> Result<String, WeaponBinaryError> {
    /*
     * #343 起改名不再由「role → 部件 ID」对照表定义，所以这里没有表可枚举了。
     *
     * 规则现在是纯结构的：把主干里的源槽位数字换成目标槽位数字，并按目标槽位归一化
     * `bs_` 前缀。它的**完整定义输入**就是下面这几项——同样的输入必然给出同样的改名
     * 结果，因此摘要仍然唯一标定「本次用的是哪条映射」。
     */
    let mut hasher = Sha256::new();
    for value in [
        MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID,
        source_root.family().as_str(),
        source_root.main_id().as_str(),
        target_main_id.as_str(),
        pair.part_id().as_str(),
        pair.part_id().role().as_str(),
    ] {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    hasher.update(MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_VERSION.to_le_bytes());
    hasher.update([0]);
    hasher.update(source_root.main_id().number().to_le_bytes());
    hasher.update(target_main_id.number().to_le_bytes());
    hasher.update([
        u8::from(source_root.main_id().has_bs_prefix()),
        u8::from(target_main_id.has_bs_prefix()),
    ]);
    Ok(format!("{:x}", hasher.finalize()))
}
