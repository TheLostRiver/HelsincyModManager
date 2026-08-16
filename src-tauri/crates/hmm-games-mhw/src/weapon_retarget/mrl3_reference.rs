use super::{WeaponBinaryError, WeaponFamily, WeaponMainId, WeaponPartRole, WeaponResourceRoot};

const MAX_GAME_RESOURCE_SEGMENTS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GameResourceReference {
    original: String,
    separator: char,
    segments: Vec<String>,
    weapon_root: Option<WeaponReferenceRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WeaponReferenceRoot {
    family: WeaponFamily,
    main_id: WeaponMainId,
    main_segment_index: usize,
}

pub(super) fn parse_game_resource_reference(
    bytes: &[u8],
) -> Result<GameResourceReference, WeaponBinaryError> {
    if bytes.is_empty()
        || !bytes.is_ascii()
        || bytes
            .first()
            .is_some_and(|byte| matches!(byte, b'/' | b'\\'))
        || bytes
            .last()
            .is_some_and(|byte| matches!(byte, b'/' | b'\\'))
        || bytes.contains(&b':')
    {
        return Err(WeaponBinaryError::ReferenceUnsafe);
    }

    let has_forward = bytes.contains(&b'/');
    let has_backward = bytes.contains(&b'\\');
    if has_forward && has_backward {
        return Err(WeaponBinaryError::ReferenceUnsafe);
    }
    let separator = if has_backward { '\\' } else { '/' };
    let value = std::str::from_utf8(bytes).map_err(|_| WeaponBinaryError::ReferenceUnsafe)?;
    let segments = value
        .split(separator)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if segments.is_empty()
        || segments.len() > MAX_GAME_RESOURCE_SEGMENTS
        || segments.iter().any(|segment| {
            segment.is_empty()
                || matches!(segment.as_str(), "." | "..")
                || !segment.bytes().all(is_safe_segment_byte)
        })
    {
        return Err(WeaponBinaryError::ReferenceUnsafe);
    }

    let logical_start = if segments
        .first()
        .is_some_and(|segment| segment == "nativePC")
    {
        if segments.len() == 1 {
            return Err(WeaponBinaryError::ReferenceUnsafe);
        }
        1
    } else {
        if segments
            .first()
            .is_some_and(|segment| segment.eq_ignore_ascii_case("nativePC"))
        {
            return Err(WeaponBinaryError::ReferenceUnsafe);
        }
        0
    };

    let weapon_root = if segments
        .get(logical_start)
        .is_some_and(|segment| segment == "wp")
    {
        if segments.len().saturating_sub(logical_start) < 4 {
            return Err(WeaponBinaryError::ReferenceUnsafe);
        }
        let family = WeaponFamily::parse(&segments[logical_start + 1])
            .map_err(|_| WeaponBinaryError::ReferenceUnsafe)?;
        let main_id = WeaponMainId::parse_for_family(&segments[logical_start + 2], family)
            .map_err(|_| WeaponBinaryError::ReferenceUnsafe)?;
        Some(WeaponReferenceRoot {
            family,
            main_id,
            main_segment_index: logical_start + 2,
        })
    } else {
        if segments
            .get(logical_start)
            .is_some_and(|segment| segment.eq_ignore_ascii_case("wp"))
        {
            return Err(WeaponBinaryError::ReferenceUnsafe);
        }
        None
    };

    Ok(GameResourceReference {
        original: value.to_owned(),
        separator,
        segments,
        weapon_root,
    })
}

impl GameResourceReference {
    pub(super) fn original(&self) -> &str {
        &self.original
    }

    pub(super) fn retarget(
        &self,
        source_root: &WeaponResourceRoot,
        target_main_id: &WeaponMainId,
    ) -> Result<Option<String>, WeaponBinaryError> {
        let Some(reference_root) = &self.weapon_root else {
            return Ok(None);
        };
        if reference_root.family != source_root.family()
            || reference_root.main_id != *source_root.main_id()
        {
            return Ok(None);
        }
        if target_main_id.family() != source_root.family() {
            return Err(WeaponBinaryError::CrossFamilyTarget);
        }

        let mut target_segments = self.segments.clone();
        target_segments[reference_root.main_segment_index] = target_main_id.as_str().to_owned();
        let mappings = part_mappings(source_root, target_main_id)?;
        let tail_start = reference_root.main_segment_index + 1;
        let tail_end = target_segments.len() - 1;
        for segment in &target_segments[tail_start..tail_end] {
            if mappings.iter().any(|(source, _)| segment.contains(source)) {
                return Err(WeaponBinaryError::ReferenceAmbiguous);
            }
        }
        target_segments[tail_end] =
            retarget_filename_segment(&target_segments[tail_end], &mappings)?;
        Ok(Some(target_segments.join(&self.separator.to_string())))
    }
}

fn part_mappings(
    source_root: &WeaponResourceRoot,
    target_main_id: &WeaponMainId,
) -> Result<Vec<(String, String)>, WeaponBinaryError> {
    let mut roles = vec![WeaponPartRole::Main];
    if let Some(secondary) = source_root.family().secondary_part() {
        roles.push(secondary.role());
    }
    roles
        .into_iter()
        .map(|role| {
            let source = source_root
                .main_id()
                .part_for_role(role)
                .map_err(|_| WeaponBinaryError::OutputInvalid)?;
            let target = target_main_id
                .part_for_role(role)
                .map_err(|_| WeaponBinaryError::CrossFamilyTarget)?;
            Ok((source.as_str().to_owned(), target.as_str().to_owned()))
        })
        .collect()
}

fn retarget_filename_segment(
    segment: &str,
    mappings: &[(String, String)],
) -> Result<String, WeaponBinaryError> {
    for (source, target) in mappings {
        if segment == source {
            return Ok(target.clone());
        }
        if let Some((stem, extension)) = segment.rsplit_once('.') {
            if stem == source && !extension.is_empty() {
                return Ok(format!("{target}.{extension}"));
            }
        }
    }
    if mappings.iter().any(|(source, _)| segment.contains(source)) {
        return Err(WeaponBinaryError::ReferenceAmbiguous);
    }
    Ok(segment.to_owned())
}

fn is_safe_segment_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
}
