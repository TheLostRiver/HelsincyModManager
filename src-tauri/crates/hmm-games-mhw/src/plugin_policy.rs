//! MHW nativePC/plugins 的 Windows x64 DLL 候选检查，只解析包内字节。
use hmm_core::{GameId, InstallTargetPath};
use hmm_ports::{GamePluginPolicy, PluginFileCheck};

#[derive(Debug, Clone, Copy, Default)]
pub struct MhwPluginPolicy;

impl GamePluginPolicy for MhwPluginPolicy {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }
    fn policy_id(&self) -> &'static str {
        "mhw.nativepc-plugin-x64"
    }
    fn policy_version(&self) -> u32 {
        1
    }

    fn is_candidate(&self, target: &InstallTargetPath) -> bool {
        let parts = target.as_str().split('/').collect::<Vec<_>>();
        parts.len() >= 3
            && parts[0].eq_ignore_ascii_case("nativePC")
            && parts[1].eq_ignore_ascii_case("plugins")
            && parts.last().is_some_and(|name| {
                name.trim_end_matches(['.', ' '])
                    .rsplit_once('.')
                    .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("dll"))
            })
    }

    fn inspect(&self, bytes: &[u8]) -> PluginFileCheck {
        inspect_pe_dll(bytes).unwrap_or(PluginFileCheck::InvalidFormat)
    }

    fn is_excluded_attachment(&self, target: &InstallTargetPath) -> bool {
        let mut parts = target.as_str().split('/');
        parts
            .next()
            .is_some_and(|root| root.eq_ignore_ascii_case("nativePC"))
            && parts
                .next_back()
                .is_some_and(crate::is_rejected_executable_file_name)
    }
}

fn inspect_pe_dll(bytes: &[u8]) -> Option<PluginFileCheck> {
    if bytes.get(..2)? != b"MZ" {
        return None;
    }
    let pe = usize::try_from(u32_at(bytes, 0x3c)?).ok()?;
    if pe < 0x40 || bytes.get(pe..pe.checked_add(4)?)? != b"PE\0\0" {
        return None;
    }
    let coff = pe.checked_add(4)?;
    if u16_at(bytes, coff)? != 0x8664 {
        return Some(PluginFileCheck::UnsupportedArchitecture);
    }
    let sections = usize::from(u16_at(bytes, coff.checked_add(2)?)?);
    let optional_size = usize::from(u16_at(bytes, coff.checked_add(16)?)?);
    let characteristics = u16_at(bytes, coff.checked_add(18)?)?;
    if characteristics & 0x2000 == 0 {
        return Some(PluginFileCheck::NotDynamicLibrary);
    }
    if characteristics & 0x0002 == 0 || sections == 0 || sections > 96 || optional_size < 112 {
        return None;
    }
    let optional = coff.checked_add(20)?;
    if u16_at(bytes, optional)? != 0x20b {
        return None;
    }
    let directory_count = usize::try_from(u32_at(bytes, optional.checked_add(108)?)?).ok()?;
    if 112usize.checked_add(directory_count.checked_mul(8)?)? > optional_size {
        return None;
    }
    let section_table = optional.checked_add(optional_size)?;
    let table_end = section_table.checked_add(sections.checked_mul(40)?)?;
    let headers_size = usize::try_from(u32_at(bytes, optional.checked_add(60)?)?).ok()?;
    if table_end > bytes.len() || headers_size < table_end || headers_size > bytes.len() {
        return None;
    }
    for index in 0..sections {
        let section = section_table.checked_add(index.checked_mul(40)?)?;
        let size = usize::try_from(u32_at(bytes, section.checked_add(16)?)?).ok()?;
        let offset = usize::try_from(u32_at(bytes, section.checked_add(20)?)?).ok()?;
        if size != 0 && (offset < headers_size || offset.checked_add(size)? > bytes.len()) {
            return None;
        }
    }
    Some(PluginFileCheck::Supported)
}

fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}
