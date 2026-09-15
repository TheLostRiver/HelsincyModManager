use std::ops::Range;

pub(super) const MAX_MATERIAL_BYTES: u64 = 256 * 1024 * 1024;
pub(super) const PATH_CAPACITY: usize = 256;
const HEADER_SIZE: usize = 40;
const TEXTURE_SIZE: usize = 272;
const MAX_TEXTURES: usize = 4096;

/// 只定位本次要改写的纹理表。模型结构、材质哈希和作者扩展数据均不参与判断。
pub(super) fn texture_fields(bytes: &[u8]) -> Result<Vec<Range<usize>>, &'static str> {
    let invalid = "equipment_material_format_invalid";
    if bytes.len() < HEADER_SIZE
        || bytes.len() as u64 > MAX_MATERIAL_BYTES
        || read_u32(bytes, 0)? != 0x004c_524d
        || read_u32(bytes, 4)? != 12
    {
        return Err(invalid);
    }
    let count = read_u32(bytes, 20)? as usize;
    if count > MAX_TEXTURES {
        return Err(invalid);
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    let start = read_offset(bytes, 24)?;
    let end = start.checked_add(count * TEXTURE_SIZE).ok_or(invalid)?;
    if start < HEADER_SIZE || end > bytes.len() {
        return Err(invalid);
    }
    if read_u32(bytes, 16)? > 0 {
        let material_start = read_offset(bytes, 32)?;
        if material_start < end || material_start > bytes.len() {
            return Err(invalid);
        }
    }
    let mut fields = Vec::with_capacity(count);
    for index in 0..count {
        let record = start + index * TEXTURE_SIZE;
        if read_u32(bytes, record)? != 0x241f_5deb {
            return Err(invalid);
        }
        let field = record + 16..record + TEXTURE_SIZE;
        if !bytes[field.clone()].contains(&0) {
            return Err(invalid);
        }
        fields.push(field);
    }
    Ok(fields)
}

pub(super) fn field_text(bytes: &[u8]) -> Option<&str> {
    let end = bytes.iter().position(|byte| *byte == 0)?;
    std::str::from_utf8(&bytes[..end]).ok()
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, &'static str> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or("equipment_material_format_invalid")?;
    Ok(u32::from_le_bytes(
        raw.try_into()
            .map_err(|_| "equipment_material_format_invalid")?,
    ))
}

fn read_offset(bytes: &[u8], offset: usize) -> Result<usize, &'static str> {
    let raw = bytes
        .get(offset..offset + 8)
        .ok_or("equipment_material_format_invalid")?;
    let value = u64::from_le_bytes(
        raw.try_into()
            .map_err(|_| "equipment_material_format_invalid")?,
    );
    usize::try_from(value).map_err(|_| "equipment_material_format_invalid")
}
