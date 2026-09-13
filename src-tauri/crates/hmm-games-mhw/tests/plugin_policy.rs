use hmm_core::InstallTargetPath;
use hmm_games_mhw::MhwPluginPolicy;
use hmm_ports::{GamePluginPolicy, PluginFileCheck};

fn synthetic_dll() -> Vec<u8> {
    let mut bytes = vec![0; 512];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[0x3c..0x40].copy_from_slice(&64u32.to_le_bytes());
    bytes[64..68].copy_from_slice(b"PE\0\0");
    bytes[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
    bytes[70..72].copy_from_slice(&1u16.to_le_bytes());
    bytes[84..86].copy_from_slice(&240u16.to_le_bytes());
    bytes[86..88].copy_from_slice(&0x2002u16.to_le_bytes());
    bytes[88..90].copy_from_slice(&0x20bu16.to_le_bytes());
    bytes[148..152].copy_from_slice(&384u32.to_le_bytes());
    bytes[344..348].copy_from_slice(&16u32.to_le_bytes());
    bytes[348..352].copy_from_slice(&384u32.to_le_bytes());
    bytes
}

#[test]
fn candidate_scope_is_only_nativepc_plugin_dlls() {
    for (value, expected) in [
        ("nativePC/plugins/fixture.dll", true),
        ("NATIVEPC/PLUGINS/author/sub/Fixture.DLL.", true),
        ("nativePC/plugins/fixture.exe", false),
        ("nativePC/plugins-extra/fixture.dll", false),
        ("nativePC/wp/two/two028/fixture.dll", false),
        ("loader.dll", false),
    ] {
        let root = value.split('/').next().unwrap();
        let target = InstallTargetPath::parse(value, [root]).unwrap();
        assert_eq!(MhwPluginPolicy.is_candidate(&target), expected, "{value}");
    }
}

#[test]
fn plugin_inspection_requires_bounded_x64_pe_dll_structure_without_execution() {
    let original = synthetic_dll();
    assert_eq!(
        MhwPluginPolicy.inspect(&original),
        PluginFileCheck::Supported
    );
    for length in [0, 1, 64, 88, 199, 327, 383, 399] {
        assert_eq!(
            MhwPluginPolicy.inspect(&original[..length]),
            PluginFileCheck::InvalidFormat,
            "truncated at {length}"
        );
    }
    for (offset, replacement, expected) in [
        (0usize, vec![0, 0], PluginFileCheck::InvalidFormat),
        (
            0x3c,
            u32::MAX.to_le_bytes().to_vec(),
            PluginFileCheck::InvalidFormat,
        ),
        (64, vec![0; 4], PluginFileCheck::InvalidFormat),
        (
            68,
            0x14cu16.to_le_bytes().to_vec(),
            PluginFileCheck::UnsupportedArchitecture,
        ),
        (
            86,
            2u16.to_le_bytes().to_vec(),
            PluginFileCheck::NotDynamicLibrary,
        ),
        (
            88,
            0x10bu16.to_le_bytes().to_vec(),
            PluginFileCheck::InvalidFormat,
        ),
        (
            196,
            u32::MAX.to_le_bytes().to_vec(),
            PluginFileCheck::InvalidFormat,
        ),
        (
            344,
            u32::MAX.to_le_bytes().to_vec(),
            PluginFileCheck::InvalidFormat,
        ),
        (
            348,
            1u32.to_le_bytes().to_vec(),
            PluginFileCheck::InvalidFormat,
        ),
    ] {
        let mut bytes = original.clone();
        bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
        assert_eq!(
            MhwPluginPolicy.inspect(&bytes),
            expected,
            "changed field at {offset}"
        );
    }
    assert_eq!(
        original,
        synthetic_dll(),
        "inspection must not rewrite the input"
    );
}
