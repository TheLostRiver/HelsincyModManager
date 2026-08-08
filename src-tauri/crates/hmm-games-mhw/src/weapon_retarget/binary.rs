use super::mrl3_reference::{parse_game_resource_reference, GameResourceReference};
use super::{WeaponModelAssetKind, WeaponModelPair, WeaponPartRole};
use crc32fast::hash as crc32;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::ops::Range;
use thiserror::Error;

pub const MHW_WEAPON_BINARY_MAX_BYTES: usize = 256 * 1024 * 1024;

const MOD3_MAGIC: u32 = 0x0044_4f4d;
const MOD3_VERSION: u16 = 237;
const MOD3_HEADER_SIZE: usize = 320;
const MOD3_BONE_ENTRY_SIZE: usize = 24;
const MOD3_BONE_MATRIX_BYTES: usize = 128;
const MOD3_BONE_REMAP_BYTES: usize = 512;
const MOD3_GROUP_ENTRY_SIZE: usize = 32;
const MOD3_MATERIAL_ENTRY_SIZE: usize = 128;
const MOD3_MESH_ENTRY_SIZE: usize = 80;
const MOD3_VERTEX_REMAP_MIN_SIZE: usize = 24;
const MOD3_MAX_BONES: usize = 4096;
const MOD3_MAX_GROUPS: usize = 4096;
const MOD3_MAX_MATERIALS: usize = 1024;
const MOD3_MAX_MESHES: usize = 8192;
const MOD3_MAX_VERTICES: u32 = 16_000_000;
const MOD3_MAX_FACE_INDICES: u32 = 48_000_000;

const MRL3_MAGIC: u32 = 0x004c_524d;
const MRL3_VERSION: u32 = 12;
const MRL3_HEADER_SIZE: usize = 40;
const MRL3_TEXTURE_ID: u32 = 0x241f_5deb;
const MRL3_TEXTURE_ENTRY_SIZE: usize = 272;
const MRL3_TEXTURE_PATH_OFFSET: usize = 16;
const MRL3_TEXTURE_PATH_CAPACITY: usize = 256;
const MRL3_MATERIAL_TYPE_ID: u32 = 0x4516_e7ab;
const MRL3_MATERIAL_ENTRY_SIZE: usize = 56;
const MRL3_RESOURCE_ENTRY_SIZE: usize = 16;
const MRL3_MAX_TEXTURES: usize = 4096;
const MRL3_MAX_MATERIALS: usize = 4096;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum WeaponBinaryError {
    #[error("MHW weapon binary format is invalid")]
    FormatInvalid,
    #[error("MHW weapon MOD3/MRL3 pair is incompatible")]
    PairIncompatible,
    #[error("MHW weapon binary reference is unsafe")]
    ReferenceUnsafe,
    #[error("MHW weapon binary reference is ambiguous")]
    ReferenceAmbiguous,
    #[error("MHW weapon binary target path is too long")]
    PathTooLong,
    #[error("MHW weapon target belongs to another family")]
    CrossFamilyTarget,
    #[error("MHW weapon transformer output is invalid")]
    OutputInvalid,
}

impl WeaponBinaryError {
    pub fn code(self) -> &'static str {
        match self {
            Self::FormatInvalid => "weapon_binary_format_invalid",
            Self::PairIncompatible => "weapon_binary_pair_incompatible",
            Self::ReferenceUnsafe => "weapon_binary_reference_unsafe",
            Self::ReferenceAmbiguous => "weapon_binary_reference_ambiguous",
            Self::PathTooLong => "weapon_binary_path_too_long",
            Self::CrossFamilyTarget => "weapon_cross_family_target",
            Self::OutputInvalid => "weapon_transformer_output_invalid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponMod3Preflight {
    version: u16,
    mesh_count: u16,
    material_count: u16,
    file_sha256: String,
    material_set_sha256: String,
}

impl WeaponMod3Preflight {
    pub fn version(&self) -> u16 {
        self.version
    }

    pub fn mesh_count(&self) -> u16 {
        self.mesh_count
    }

    pub fn material_count(&self) -> u16 {
        self.material_count
    }

    pub fn file_sha256(&self) -> &str {
        &self.file_sha256
    }

    pub fn material_set_sha256(&self) -> &str {
        &self.material_set_sha256
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponMrl3Preflight {
    version: u32,
    texture_count: u32,
    material_count: u32,
    file_sha256: String,
    material_set_sha256: String,
}

impl WeaponMrl3Preflight {
    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn texture_count(&self) -> u32 {
        self.texture_count
    }

    pub fn material_count(&self) -> u32 {
        self.material_count
    }

    pub fn file_sha256(&self) -> &str {
        &self.file_sha256
    }

    pub fn material_set_sha256(&self) -> &str {
        &self.material_set_sha256
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponModelPairPreflight {
    part_role: WeaponPartRole,
    material_count: u32,
    mod3_file_sha256: String,
    mrl3_file_sha256: String,
    material_set_sha256: String,
}

impl WeaponModelPairPreflight {
    pub fn part_role(&self) -> WeaponPartRole {
        self.part_role
    }

    pub fn material_count(&self) -> u32 {
        self.material_count
    }

    pub fn mod3_file_sha256(&self) -> &str {
        &self.mod3_file_sha256
    }

    pub fn mrl3_file_sha256(&self) -> &str {
        &self.mrl3_file_sha256
    }

    pub fn material_set_sha256(&self) -> &str {
        &self.material_set_sha256
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedTextureReference {
    pub(super) field_range: Range<usize>,
    pub(super) reference: Option<GameResourceReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedMod3 {
    pub(super) report: WeaponMod3Preflight,
    pub(super) material_hashes: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedMrl3 {
    pub(super) report: WeaponMrl3Preflight,
    pub(super) material_hashes: Vec<u32>,
    pub(super) textures: Vec<ParsedTextureReference>,
}

pub fn preflight_mhw_weapon_mod3(bytes: &[u8]) -> Result<WeaponMod3Preflight, WeaponBinaryError> {
    Ok(parse_mod3(bytes)?.report)
}

pub fn preflight_mhw_weapon_mrl3(bytes: &[u8]) -> Result<WeaponMrl3Preflight, WeaponBinaryError> {
    Ok(parse_mrl3(bytes)?.report)
}

pub fn preflight_mhw_weapon_model_pair(
    pair: &WeaponModelPair,
    mod3_bytes: &[u8],
    mrl3_bytes: &[u8],
) -> Result<WeaponModelPairPreflight, WeaponBinaryError> {
    let (mod3, mrl3) = parse_model_pair(pair, mod3_bytes, mrl3_bytes)?;
    Ok(pair_report(pair, &mod3, &mrl3))
}

pub(super) fn parse_model_pair(
    pair: &WeaponModelPair,
    mod3_bytes: &[u8],
    mrl3_bytes: &[u8],
) -> Result<(ParsedMod3, ParsedMrl3), WeaponBinaryError> {
    if pair.mod3().kind() != WeaponModelAssetKind::Mod3
        || pair.mrl3().kind() != WeaponModelAssetKind::Mrl3
        || pair.mod3().model_path().root() != pair.mrl3().model_path().root()
        || pair.mod3().model_path().part_id() != pair.mrl3().model_path().part_id()
        || pair.part_id() != pair.mod3().model_path().part_id()
    {
        return Err(WeaponBinaryError::PairIncompatible);
    }

    let mod3 = parse_mod3(mod3_bytes)?;
    let mrl3 = parse_mrl3(mrl3_bytes)?;
    if mod3.material_hashes != mrl3.material_hashes {
        return Err(WeaponBinaryError::PairIncompatible);
    }
    Ok((mod3, mrl3))
}

fn pair_report(
    pair: &WeaponModelPair,
    mod3: &ParsedMod3,
    mrl3: &ParsedMrl3,
) -> WeaponModelPairPreflight {
    WeaponModelPairPreflight {
        part_role: pair.part_id().role(),
        material_count: mrl3.report.material_count,
        mod3_file_sha256: mod3.report.file_sha256.clone(),
        mrl3_file_sha256: mrl3.report.file_sha256.clone(),
        material_set_sha256: mod3.report.material_set_sha256.clone(),
    }
}

fn parse_mod3(bytes: &[u8]) -> Result<ParsedMod3, WeaponBinaryError> {
    validate_file_envelope(bytes, MOD3_HEADER_SIZE)?;
    if read_u32(bytes, 0)? != MOD3_MAGIC || read_u16(bytes, 4)? != MOD3_VERSION {
        return Err(WeaponBinaryError::FormatInvalid);
    }

    let bone_count = usize::from(read_u16(bytes, 6)?);
    let mesh_count_u16 = read_u16(bytes, 8)?;
    let mesh_count = usize::from(mesh_count_u16);
    let material_count_u16 = read_u16(bytes, 10)?;
    let material_count = usize::from(material_count_u16);
    let vertex_count = read_u32(bytes, 12)?;
    let face_count = read_u32(bytes, 16)?;
    let vertex_buffer_size = to_usize(read_u64(bytes, 24)?)?;
    let group_count =
        usize::try_from(read_u32(bytes, 32)?).map_err(|_| WeaponBinaryError::FormatInvalid)?;

    if mesh_count == 0
        || material_count == 0
        || bone_count > MOD3_MAX_BONES
        || mesh_count > MOD3_MAX_MESHES
        || material_count > MOD3_MAX_MATERIALS
        || group_count > MOD3_MAX_GROUPS
        || vertex_count > MOD3_MAX_VERTICES
        || face_count > MOD3_MAX_FACE_INDICES
        || vertex_buffer_size > MHW_WEAPON_BINARY_MAX_BYTES
    {
        return Err(WeaponBinaryError::FormatInvalid);
    }

    let bone_offset = read_u64(bytes, 48)?;
    let group_offset = read_u64(bytes, 56)?;
    let material_offset = read_u64(bytes, 64)?;
    let mesh_offset = read_u64(bytes, 72)?;
    let vertex_offset = read_u64(bytes, 80)?;
    let face_offset = read_u64(bytes, 88)?;
    let vertex_remap_offset = read_u64(bytes, 96)?;
    let unknown_offset = read_u64(bytes, 104)?;

    let mut ranges = Vec::new();
    let bone_bytes = bone_count
        .checked_mul(MOD3_BONE_ENTRY_SIZE + MOD3_BONE_MATRIX_BYTES)
        .and_then(|value| value.checked_add(MOD3_BONE_REMAP_BYTES))
        .and_then(align_16)
        .ok_or(WeaponBinaryError::FormatInvalid)?;
    push_optional_counted_range(
        &mut ranges,
        bone_offset,
        bone_count,
        bone_bytes,
        bytes.len(),
    )?;
    push_optional_counted_range(
        &mut ranges,
        group_offset,
        group_count,
        checked_mul(group_count, MOD3_GROUP_ENTRY_SIZE)?,
        bytes.len(),
    )?;
    let material_range = required_range(
        material_offset,
        checked_mul(material_count, MOD3_MATERIAL_ENTRY_SIZE)?,
        bytes.len(),
    )?;
    ranges.push(material_range.clone());
    let mesh_range = required_range(
        mesh_offset,
        checked_mul(mesh_count, MOD3_MESH_ENTRY_SIZE)?,
        bytes.len(),
    )?;
    ranges.push(mesh_range.clone());
    let vertex_range = required_range(vertex_offset, vertex_buffer_size, bytes.len())?;
    ranges.push(vertex_range);
    let face_buffer_size = usize::try_from(face_count)
        .ok()
        .and_then(|count| count.checked_mul(2))
        .ok_or(WeaponBinaryError::FormatInvalid)?;
    let face_range = required_range(face_offset, face_buffer_size, bytes.len())?;
    ranges.push(face_range);
    let remap_range = required_range(vertex_remap_offset, MOD3_VERTEX_REMAP_MIN_SIZE, bytes.len())?;
    ranges.push(remap_range);
    if unknown_offset != 0 {
        let offset = to_usize(unknown_offset)?;
        if !(MOD3_HEADER_SIZE..=bytes.len()).contains(&offset) {
            return Err(WeaponBinaryError::FormatInvalid);
        }
    }
    validate_non_overlapping_ranges(&ranges, MOD3_HEADER_SIZE)?;
    validate_monotonic_offsets(&[
        (bone_count > 0, bone_offset),
        (group_count > 0, group_offset),
        (true, material_offset),
        (true, mesh_offset),
        (true, vertex_offset),
        (true, face_offset),
        (true, vertex_remap_offset),
    ])?;

    let mut material_hashes = BTreeSet::new();
    for index in 0..material_count {
        let start = material_range.start + index * MOD3_MATERIAL_ENTRY_SIZE;
        let field = &bytes[start..start + MOD3_MATERIAL_ENTRY_SIZE];
        let nul = field
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(WeaponBinaryError::FormatInvalid)?;
        let name = &field[..nul];
        if name.is_empty()
            || !name
                .iter()
                .all(|byte| byte.is_ascii() && (0x20..=0x7e).contains(byte))
            || name.first() == Some(&b' ')
            || name.last() == Some(&b' ')
            || !material_hashes.insert(crc32(name) ^ u32::MAX)
        {
            return Err(WeaponBinaryError::FormatInvalid);
        }
    }

    let vertex_buffer_size_u64 =
        u64::try_from(vertex_buffer_size).map_err(|_| WeaponBinaryError::FormatInvalid)?;
    let face_buffer_size_u64 =
        u64::try_from(face_buffer_size).map_err(|_| WeaponBinaryError::FormatInvalid)?;
    for index in 0..mesh_count {
        let start = mesh_range.start + index * MOD3_MESH_ENTRY_SIZE;
        let mesh_vertex_count = u64::from(read_u16(bytes, start + 2)?);
        let material_id = usize::from(read_u16(bytes, start + 6)?);
        let block_size = u64::from(bytes[start + 14]);
        let vertex_sub = u64::from(read_u32(bytes, start + 16)?);
        let mesh_vertex_offset = u64::from(read_u32(bytes, start + 20)?);
        let before_face_count = u64::from(read_u32(bytes, start + 28)?);
        let mesh_face_count = u64::from(read_u32(bytes, start + 32)?);
        let vertex_base = u64::from(read_u32(bytes, start + 36)?);
        let before_vertex_count = u64::from(read_u32(bytes, start + 60)?);

        if material_id >= material_count
            || mesh_vertex_count == 0
            || mesh_face_count == 0
            || mesh_face_count % 3 != 0
            || block_size == 0
            || before_vertex_count
                .checked_add(mesh_vertex_count)
                .is_none_or(|end| end > u64::from(vertex_count))
        {
            return Err(WeaponBinaryError::FormatInvalid);
        }
        let vertex_end = vertex_sub
            .checked_add(vertex_base)
            .and_then(|base| base.checked_mul(block_size))
            .and_then(|relative| relative.checked_add(mesh_vertex_offset))
            .and_then(|start| {
                mesh_vertex_count
                    .checked_mul(block_size)
                    .and_then(|length| start.checked_add(length))
            })
            .ok_or(WeaponBinaryError::FormatInvalid)?;
        let face_end = before_face_count
            .checked_add(mesh_face_count)
            .and_then(|count| count.checked_mul(2))
            .ok_or(WeaponBinaryError::FormatInvalid)?;
        if vertex_end > vertex_buffer_size_u64 || face_end > face_buffer_size_u64 {
            return Err(WeaponBinaryError::FormatInvalid);
        }
    }

    let material_hashes = material_hashes.into_iter().collect::<Vec<_>>();
    let material_set_sha256 = material_set_sha256(&material_hashes);
    Ok(ParsedMod3 {
        report: WeaponMod3Preflight {
            version: MOD3_VERSION,
            mesh_count: mesh_count_u16,
            material_count: material_count_u16,
            file_sha256: sha256_hex(bytes),
            material_set_sha256,
        },
        material_hashes,
    })
}

fn parse_mrl3(bytes: &[u8]) -> Result<ParsedMrl3, WeaponBinaryError> {
    validate_file_envelope(bytes, MRL3_HEADER_SIZE)?;
    if read_u32(bytes, 0)? != MRL3_MAGIC || read_u32(bytes, 4)? != MRL3_VERSION {
        return Err(WeaponBinaryError::FormatInvalid);
    }

    let material_count_u32 = read_u32(bytes, 16)?;
    let texture_count_u32 = read_u32(bytes, 20)?;
    let material_count =
        usize::try_from(material_count_u32).map_err(|_| WeaponBinaryError::FormatInvalid)?;
    let texture_count =
        usize::try_from(texture_count_u32).map_err(|_| WeaponBinaryError::FormatInvalid)?;
    if material_count == 0
        || material_count > MRL3_MAX_MATERIALS
        || texture_count > MRL3_MAX_TEXTURES
    {
        return Err(WeaponBinaryError::FormatInvalid);
    }

    let texture_range = required_range(
        read_u64(bytes, 24)?,
        checked_mul(texture_count, MRL3_TEXTURE_ENTRY_SIZE)?,
        bytes.len(),
    )?;
    let material_range = required_range(
        read_u64(bytes, 32)?,
        checked_mul(material_count, MRL3_MATERIAL_ENTRY_SIZE)?,
        bytes.len(),
    )?;
    validate_non_overlapping_ranges(
        &[texture_range.clone(), material_range.clone()],
        MRL3_HEADER_SIZE,
    )?;
    if texture_range.end > material_range.start {
        return Err(WeaponBinaryError::FormatInvalid);
    }

    let mut textures = Vec::with_capacity(texture_count);
    for index in 0..texture_count {
        let record = texture_range.start + index * MRL3_TEXTURE_ENTRY_SIZE;
        if read_u32(bytes, record)? != MRL3_TEXTURE_ID {
            return Err(WeaponBinaryError::FormatInvalid);
        }
        let start = record + MRL3_TEXTURE_PATH_OFFSET;
        let field_range = start..start + MRL3_TEXTURE_PATH_CAPACITY;
        let field = &bytes[field_range.clone()];
        let nul = field
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(WeaponBinaryError::FormatInvalid)?;
        let reference = if nul == 0 {
            None
        } else {
            Some(parse_game_resource_reference(&field[..nul])?)
        };
        textures.push(ParsedTextureReference {
            field_range,
            reference,
        });
    }

    let resource_floor = align_16(material_range.end).ok_or(WeaponBinaryError::FormatInvalid)?;
    let mut material_hashes = BTreeSet::new();
    let mut resource_ranges = Vec::new();
    for index in 0..material_count {
        let record = material_range.start + index * MRL3_MATERIAL_ENTRY_SIZE;
        if read_u32(bytes, record)? != MRL3_MATERIAL_TYPE_ID {
            return Err(WeaponBinaryError::FormatInvalid);
        }
        let material_hash = read_u32(bytes, record + 4)?;
        let block_size = usize::try_from(read_u32(bytes, record + 16)?)
            .map_err(|_| WeaponBinaryError::FormatInvalid)?;
        let resource_count = usize::from(read_u16(bytes, record + 22)?);
        let block_offset = read_u64(bytes, record + 48)?;
        if !material_hashes.insert(material_hash) || resource_count % 2 != 0 {
            return Err(WeaponBinaryError::FormatInvalid);
        }
        if block_size == 0 {
            if block_offset != 0 || resource_count != 0 {
                return Err(WeaponBinaryError::FormatInvalid);
            }
            continue;
        }
        let minimum_size = checked_mul(resource_count / 2, MRL3_RESOURCE_ENTRY_SIZE)?;
        let range = required_range(block_offset, block_size, bytes.len())?;
        if block_size < minimum_size || range.start < resource_floor || range.start % 16 != 0 {
            return Err(WeaponBinaryError::FormatInvalid);
        }
        resource_ranges.push(range);
    }
    validate_non_overlapping_ranges(&resource_ranges, resource_floor)?;

    let material_hashes = material_hashes.into_iter().collect::<Vec<_>>();
    let material_set_sha256 = material_set_sha256(&material_hashes);
    Ok(ParsedMrl3 {
        report: WeaponMrl3Preflight {
            version: MRL3_VERSION,
            texture_count: texture_count_u32,
            material_count: material_count_u32,
            file_sha256: sha256_hex(bytes),
            material_set_sha256,
        },
        material_hashes,
        textures,
    })
}

fn validate_file_envelope(bytes: &[u8], minimum_size: usize) -> Result<(), WeaponBinaryError> {
    if bytes.len() < minimum_size || bytes.len() > MHW_WEAPON_BINARY_MAX_BYTES {
        Err(WeaponBinaryError::FormatInvalid)
    } else {
        Ok(())
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, WeaponBinaryError> {
    let raw = bytes
        .get(offset..offset + 2)
        .ok_or(WeaponBinaryError::FormatInvalid)?;
    Ok(u16::from_le_bytes([raw[0], raw[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, WeaponBinaryError> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or(WeaponBinaryError::FormatInvalid)?;
    Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, WeaponBinaryError> {
    let raw = bytes
        .get(offset..offset + 8)
        .ok_or(WeaponBinaryError::FormatInvalid)?;
    Ok(u64::from_le_bytes([
        raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
    ]))
}

fn to_usize(value: u64) -> Result<usize, WeaponBinaryError> {
    usize::try_from(value).map_err(|_| WeaponBinaryError::FormatInvalid)
}

fn checked_mul(left: usize, right: usize) -> Result<usize, WeaponBinaryError> {
    left.checked_mul(right)
        .ok_or(WeaponBinaryError::FormatInvalid)
}

fn align_16(value: usize) -> Option<usize> {
    value.checked_add(15).map(|aligned| aligned & !15)
}

fn required_range(
    offset: u64,
    length: usize,
    file_len: usize,
) -> Result<Range<usize>, WeaponBinaryError> {
    let start = to_usize(offset)?;
    let end = start
        .checked_add(length)
        .ok_or(WeaponBinaryError::FormatInvalid)?;
    if start < MRL3_HEADER_SIZE || end > file_len {
        return Err(WeaponBinaryError::FormatInvalid);
    }
    Ok(start..end)
}

fn push_optional_counted_range(
    ranges: &mut Vec<Range<usize>>,
    offset: u64,
    count: usize,
    length: usize,
    file_len: usize,
) -> Result<(), WeaponBinaryError> {
    if count == 0 {
        if offset != 0 {
            return Err(WeaponBinaryError::FormatInvalid);
        }
        return Ok(());
    }
    ranges.push(required_range(offset, length, file_len)?);
    Ok(())
}

fn validate_non_overlapping_ranges(
    ranges: &[Range<usize>],
    minimum_start: usize,
) -> Result<(), WeaponBinaryError> {
    let mut sorted = ranges.to_vec();
    sorted.sort_by_key(|range| (range.start, range.end));
    let mut previous_end = minimum_start;
    for range in sorted {
        if range.start < minimum_start || range.start < previous_end || range.end < range.start {
            return Err(WeaponBinaryError::FormatInvalid);
        }
        previous_end = range.end;
    }
    Ok(())
}

fn validate_monotonic_offsets(offsets: &[(bool, u64)]) -> Result<(), WeaponBinaryError> {
    let mut previous = u64::try_from(MOD3_HEADER_SIZE).expect("MOD3 header fits u64");
    for (present, offset) in offsets {
        if *present {
            if *offset < previous {
                return Err(WeaponBinaryError::FormatInvalid);
            }
            previous = *offset;
        } else if *offset != 0 {
            return Err(WeaponBinaryError::FormatInvalid);
        }
    }
    Ok(())
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn material_set_sha256(hashes: &[u32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"hmm-mhw-weapon-material-set-v1\0");
    for hash in hashes {
        hasher.update(hash.to_le_bytes());
    }
    format!("{:x}", hasher.finalize())
}
