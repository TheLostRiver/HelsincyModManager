use super::part_rename::{rename_part_prefix, PartRename};
use super::{WeaponBinaryError, WeaponFamily, WeaponMainId, WeaponResourceRoot};

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

/// 解析一条 MRL3 贴图引用。
///
/// **畸形或可疑的引用仍然硬失败**（驱动器前缀、父目录穿越、首尾分隔符、混用分隔符、
/// 控制字符、非 ASCII）——那是真实的损坏/可疑信号，且不是任何真实 Mod 的形态。
///
/// 变化的只是「**不是我们的引用**」这一类：见 [`WeaponReferenceRoot`] 的判定。
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

    /*
     * 「是否武器根」是一次**定向前缀匹配**，不是语法校验。
     *
     * #336 的 L3：旧版一旦看到首段是 `wp`，就要求后面必须是 `<已知族>/<合法主 ID>`，
     * 否则报 `ReferenceUnsafe` 让整个 MRL3 解析失败。可真实世界里想让 Mod 可重定向的
     * 作者**本来就**把贴图放在与槽位无关的目录，这些引用全都长这样：
     *
     *   wp\swo\Tamonowo\Tamonowo_BML    （族级作者目录）
     *   wp\two\DARKMOON\DARKMOON_BML    （同上）
     *   wp\two\textures\opulent_BML     （同上）
     *   wp\Sakurad\Sakurad_BML          （只 3 段，连族段都不是）
     *
     * 它们不是损坏，只是「不属于我们要改写的槽位」。记 `None` 后原样保留，一个字节都不写。
     */
    let weapon_root = segments
        .get(logical_start)
        .filter(|segment| *segment == "wp")
        .and_then(|_| {
            let family = WeaponFamily::parse(segments.get(logical_start + 1)?).ok()?;
            let main_id =
                WeaponMainId::parse_for_family(segments.get(logical_start + 2)?, family).ok()?;
            // 主 ID 之后至少还要有一段文件名，否则没有可改写的目标。
            (segments.len() > logical_start + 3).then_some(WeaponReferenceRoot {
                family,
                main_id,
                main_segment_index: logical_start + 2,
            })
        });

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
    // 与磁盘文件重定位共用同一张对照表（part_rename::part_mappings），保证两处改名一致。
    super::part_rename::part_mappings(source_root.main_id(), target_main_id)
        .map_err(|_| WeaponBinaryError::CrossFamilyTarget)
}

fn retarget_filename_segment(
    segment: &str,
    mappings: &[(String, String)],
) -> Result<String, WeaponBinaryError> {
    // 统一走前缀替换规则（见 part_rename）。旧版只认「整段相等」与「去扩展名后相等」，
    // 真实贴图名 `two003_BML` 只「包含」部件 ID，会被判 ambiguous——#336 的 L5。
    match rename_part_prefix(segment, mappings) {
        PartRename::Renamed(renamed) => Ok(renamed),
        PartRename::Unrelated => Ok(segment.to_owned()),
        PartRename::Ambiguous => Err(WeaponBinaryError::ReferenceAmbiguous),
    }
}

/// 引用段允许的字节。
///
/// `[` `]` 是**原版**资源名就在用的字符（`Assets\default_tex\CM\country_road_hor[1]_CM-00`），
/// 旧版不允许它们，导致带这类引用的 MRL3 整个解析失败——#336 的 L4。放行是安全的：
/// 这类引用不匹配 `wp/<族>/<主 ID>/` 前缀，永远不会被改写，字节原样保留。
fn is_safe_segment_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'[' | b']')
}
