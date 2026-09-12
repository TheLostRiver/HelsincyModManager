//! 默认路径策略的组合映射；旧材质转换器继续使用原有路径和引用规则。
use super::family::{WeaponFamily, WeaponMainId};
use super::part_rename::{rename_weapon_stem, PartRename};
use super::path::{WeaponPathError, WeaponResourceRoot};
use hmm_core::{InstallTargetPath, RetargetFileReason};
use std::borrow::Cow;
use std::collections::BTreeSet;

pub(crate) enum WeaponResourceMapping {
    Relocated(InstallTargetPath),
    Kept(RetargetFileReason),
}

/// 部件目录只能由同源模型／材质的完整文件主干证明，不能从目录自身的数字反推。
pub(crate) struct WeaponResourceMapper<'a> {
    root: &'a WeaponResourceRoot,
    part_stems: BTreeSet<String>,
    // 候选模型提供的前缀仅用于识别矛盾，不直接授权目录迁移。
    part_prefixes: BTreeSet<String>,
}

impl<'a> WeaponResourceMapper<'a> {
    pub(crate) fn new(
        root: &'a WeaponResourceRoot,
        paths: impl IntoIterator<Item = &'a InstallTargetPath>,
    ) -> Self {
        let mut candidates = Vec::new();
        let mut part_prefixes = BTreeSet::new();
        for path in paths {
            if !root.contains(path) {
                continue;
            }
            let parts = path.as_str().split('/').collect::<Vec<_>>();
            let Some((stem, extension)) = parts.last().and_then(|name| name.rsplit_once('.'))
            else {
                continue;
            };
            if !["mod3", "mrl3"]
                .iter()
                .any(|ext| extension.eq_ignore_ascii_case(ext))
                || identity_issue(stem, root.main_id()).is_some()
                || parts[4..parts.len() - 1]
                    .iter()
                    .any(|part| explicit_main_conflict(part, root.main_id()))
            {
                continue;
            }
            if let PartRename::Renamed(_) =
                rename_weapon_stem(&normalize_bs(stem), root.main_id(), root.main_id())
            {
                candidates.push((stem, path));
                if let Some(token) = numbered_tokens(stem).first() {
                    part_prefixes.insert(token.prefix.to_ascii_lowercase());
                }
            }
        }
        let mut mapper = Self {
            root,
            part_stems: BTreeSet::new(),
            part_prefixes,
        };
        // 先收集身份，再核对完整候选路径，避免扫描顺序决定一个矛盾文件能否提供证据。
        for (stem, path) in candidates {
            let parts = path.as_str().split('/').collect::<Vec<_>>();
            if parts[4..parts.len() - 1]
                .iter()
                .all(|part| mapper.directory_issue(part).is_none())
            {
                mapper.part_stems.insert(stem.to_ascii_lowercase());
            }
        }
        mapper
    }

    pub(crate) fn map(
        &self,
        path: &InstallTargetPath,
        target: &WeaponMainId,
    ) -> Result<WeaponResourceMapping, WeaponPathError> {
        if target.family() != self.root.family() {
            return Err(WeaponPathError::CrossFamilyTarget);
        }
        if !self.root.contains(path) {
            return Err(WeaponPathError::NotWeaponPath);
        }
        let parts = path.as_str().split('/').collect::<Vec<_>>();
        let filename = parts.last().ok_or(WeaponPathError::UnsafePath)?;
        // 先核对整条路径，再一次性生成目标。不能移动目录后才发现文件名指向别的装备。
        let stem = filename
            .rsplit_once('.')
            .map_or(*filename, |(stem, _)| stem);
        if let Some(reason) = identity_issue(stem, self.root.main_id()) {
            return Ok(WeaponResourceMapping::Kept(reason));
        }
        let mut directories = Vec::new();
        let mut mapped_directory = false;
        for part in &parts[4..parts.len() - 1] {
            if let Some(reason) = self.directory_issue(part) {
                return Ok(WeaponResourceMapping::Kept(reason));
            }
            if part.eq_ignore_ascii_case(self.root.main_id().as_str())
                || self.part_stems.contains(&part.to_ascii_lowercase())
            {
                match rename_weapon_stem(&normalize_bs(part), self.root.main_id(), target) {
                    PartRename::Renamed(name) => {
                        directories.push(name);
                        mapped_directory = true;
                    }
                    PartRename::Unrelated | PartRename::Ambiguous => {
                        return Ok(WeaponResourceMapping::Kept(
                            RetargetFileReason::AmbiguousResourceIdentity,
                        ));
                    }
                }
            } else {
                directories.push((*part).to_owned());
            }
        }
        let filename =
            match rename_weapon_stem(&normalize_bs(filename), self.root.main_id(), target) {
                PartRename::Renamed(name) => name,
                PartRename::Unrelated if mapped_directory => (*filename).to_owned(),
                PartRename::Unrelated => {
                    return Ok(WeaponResourceMapping::Kept(
                        RetargetFileReason::UnmappedResource,
                    ))
                }
                PartRename::Ambiguous => {
                    return Ok(WeaponResourceMapping::Kept(
                        RetargetFileReason::AmbiguousResourceIdentity,
                    ))
                }
            };
        let mut mapped = parts[..3]
            .iter()
            .map(|part| (*part).to_owned())
            .collect::<Vec<_>>();
        mapped.push(target.as_str().to_owned());
        mapped.extend(directories);
        mapped.push(filename);
        InstallTargetPath::parse(mapped.join("/"), ["nativePC"])
            .map(WeaponResourceMapping::Relocated)
            .map_err(|_| WeaponPathError::UnsafePath)
    }

    fn directory_issue(&self, name: &str) -> Option<RetargetFileReason> {
        if explicit_main_conflict(name, self.root.main_id()) {
            return Some(RetargetFileReason::ConflictingResourceIdentity);
        }
        let known_part = numbered_tokens(name).first().is_some_and(|token| {
            self.part_prefixes
                .contains(&token.prefix.to_ascii_lowercase())
                || self
                    .root
                    .family()
                    .secondary_part()
                    .is_some_and(|part| token.prefix.eq_ignore_ascii_case(part.prefix()))
        });
        if known_part {
            identity_issue(name, self.root.main_id())
        } else {
            None
        }
    }
}

fn normalize_bs(name: &str) -> Cow<'_, str> {
    if name
        .get(..3)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("bs_"))
    {
        Cow::Owned(format!("bs_{}", &name[3..]))
    } else {
        Cow::Borrowed(name)
    }
}

fn explicit_main_conflict(name: &str, source: &WeaponMainId) -> bool {
    name.is_ascii() && WeaponMainId::parse(&name.to_ascii_lowercase()).is_ok_and(|id| id != *source)
}

struct NumberedToken<'a> {
    prefix: &'a str,
    digits: &'a str,
    has_bs: bool,
}

/// 只识别字母／下划线后完整的三位数字段。长数字、日期及纯数字不构成装备编号。
/// 这些 token 仅用于拒绝相互矛盾的身份；真正改名仍交给共享的 part_rename 规则。
fn numbered_tokens(name: &str) -> Vec<NumberedToken<'_>> {
    let bytes = name.as_bytes();
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if !bytes[cursor].is_ascii_digit() {
            cursor += 1;
            continue;
        }
        let start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor - start != 3 {
            continue;
        }
        let mut prefix_start = start;
        while prefix_start > 0
            && (bytes[prefix_start - 1].is_ascii_alphabetic() || bytes[prefix_start - 1] == b'_')
        {
            prefix_start -= 1;
        }
        let prefix = name[prefix_start..start].trim_start_matches('_');
        if prefix.is_empty() {
            continue;
        }
        let has_bs = prefix
            .get(..3)
            .is_some_and(|part| part.eq_ignore_ascii_case("bs_"));
        tokens.push(NumberedToken {
            prefix: if has_bs { &prefix[3..] } else { prefix },
            digits: &name[start..cursor],
            has_bs,
        });
    }
    tokens
}

fn identity_issue(name: &str, source: &WeaponMainId) -> Option<RetargetFileReason> {
    let tokens = numbered_tokens(name);
    let source_digits = format!("{:03}", source.number());
    if tokens.iter().any(|token| {
        token.digits != source_digits
            || token.has_bs != source.has_bs_prefix()
            || WeaponFamily::parse(&token.prefix.to_ascii_lowercase())
                .is_ok_and(|family| family != source.family())
    }) {
        Some(RetargetFileReason::ConflictingResourceIdentity)
    } else if tokens.len() > 1 {
        Some(RetargetFileReason::AmbiguousResourceIdentity)
    } else {
        None
    }
}
