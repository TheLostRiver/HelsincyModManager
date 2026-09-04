use hmm_core::PackageFileId;
use hmm_games_mhw::{
    analyze_mhw_weapon_assets, build_mhw_weapon_mrl3_transform_invocation,
    preflight_mhw_weapon_mod3, preflight_mhw_weapon_model_pair, preflight_mhw_weapon_mrl3,
    transform_mhw_weapon_mrl3_texture_paths, MhwWeaponMrl3TexturePathTransformer,
    WeaponBinaryError, WeaponMainId, WeaponModelPair, WeaponPartRole,
    MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID, MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_VERSION,
};
use hmm_ports::{ContentTransformRequest, ContentTransformer, ReplacementAsset};
use std::collections::BTreeMap;

const MOD3_HEADER_SIZE: usize = 320;
const MOD3_MATERIAL_ENTRY_SIZE: usize = 128;
const MOD3_MESH_ENTRY_SIZE: usize = 80;
const MRL3_HEADER_SIZE: usize = 40;
const MRL3_TEXTURE_ENTRY_SIZE: usize = 272;
const MRL3_MATERIAL_ENTRY_SIZE: usize = 56;
const MRL3_TEXTURE_PATH_OFFSET: usize = 16;
const MRL3_TEXTURE_PATH_CAPACITY: usize = 256;
const ARTIFICIAL_MATERIAL_HASH: u32 = 0xa7f6_8bf8;
const SECOND_ARTIFICIAL_MATERIAL_HASH: u32 = 0x16e2_7268;

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn align_16(value: usize) -> usize {
    (value + 15) & !15
}

fn pair(main_id: &str, part_id: &str) -> WeaponModelPair {
    let family = main_id
        .strip_prefix("bs_")
        .unwrap_or(main_id)
        .trim_end_matches(|character: char| character.is_ascii_digit());
    let assets = [
        ReplacementAsset::new(
            PackageFileId::new("artificial-mod3"),
            format!("nativePC/wp/{family}/{main_id}/mod/{part_id}.mod3"),
        ),
        ReplacementAsset::new(
            PackageFileId::new("artificial-mrl3"),
            format!("nativePC/wp/{family}/{main_id}/mod/{part_id}.mrl3"),
        ),
    ];
    analyze_mhw_weapon_assets(&assets)
        .expect("artificial pair closure")
        .pairs()[0]
        .clone()
}

fn artificial_mod3(material_names: &[&str]) -> Vec<u8> {
    assert!(!material_names.is_empty());
    assert!(material_names
        .iter()
        .all(|name| !name.is_empty() && name.len() < 128));

    let material_offset = MOD3_HEADER_SIZE;
    let mesh_offset = material_offset + material_names.len() * MOD3_MATERIAL_ENTRY_SIZE;
    let vertex_offset = mesh_offset + MOD3_MESH_ENTRY_SIZE + 4;
    let vertex_buffer_size = 36usize;
    let face_offset = vertex_offset + vertex_buffer_size;
    let face_buffer_size = 8usize;
    let vertex_remap_offset = face_offset + face_buffer_size;
    let mut bytes = vec![0u8; vertex_remap_offset + 24];

    write_u32(&mut bytes, 0, 0x0044_4f4d);
    write_u16(&mut bytes, 4, 237);
    write_u16(&mut bytes, 8, 1);
    write_u16(
        &mut bytes,
        10,
        u16::try_from(material_names.len()).expect("artificial material count"),
    );
    write_u32(&mut bytes, 12, 3);
    write_u32(&mut bytes, 16, 3);
    write_u64(&mut bytes, 24, vertex_buffer_size as u64);
    write_u64(&mut bytes, 64, material_offset as u64);
    write_u64(&mut bytes, 72, mesh_offset as u64);
    write_u64(&mut bytes, 80, vertex_offset as u64);
    write_u64(&mut bytes, 88, face_offset as u64);
    write_u64(&mut bytes, 96, vertex_remap_offset as u64);

    for (index, name) in material_names.iter().enumerate() {
        let start = material_offset + index * MOD3_MATERIAL_ENTRY_SIZE;
        bytes[start..start + name.len()].copy_from_slice(name.as_bytes());
    }

    write_u16(&mut bytes, mesh_offset + 2, 3);
    write_u16(&mut bytes, mesh_offset + 6, 0);
    write_u16(&mut bytes, mesh_offset + 8, 1);
    bytes[mesh_offset + 14] = 12;
    write_u32(&mut bytes, mesh_offset + 32, 3);
    write_u32(&mut bytes, vertex_remap_offset, 4);
    bytes
}

fn artificial_mrl3(paths: &[&str], material_hashes: &[u32]) -> Vec<u8> {
    let texture_offset = MRL3_HEADER_SIZE;
    let material_offset = texture_offset + paths.len() * MRL3_TEXTURE_ENTRY_SIZE;
    let material_end = material_offset + material_hashes.len() * MRL3_MATERIAL_ENTRY_SIZE;
    let resource_offset = align_16(material_end);
    let mut bytes = vec![0u8; resource_offset + material_hashes.len() * 16];

    write_u32(&mut bytes, 0, 0x004c_524d);
    write_u32(&mut bytes, 4, 12);
    write_u32(
        &mut bytes,
        16,
        u32::try_from(material_hashes.len()).expect("artificial material count"),
    );
    write_u32(
        &mut bytes,
        20,
        u32::try_from(paths.len()).expect("artificial texture count"),
    );
    write_u64(&mut bytes, 24, texture_offset as u64);
    write_u64(&mut bytes, 32, material_offset as u64);

    for (index, path) in paths.iter().enumerate() {
        assert!(path.len() < MRL3_TEXTURE_PATH_CAPACITY);
        let record = texture_offset + index * MRL3_TEXTURE_ENTRY_SIZE;
        write_u32(&mut bytes, record, 0x241f_5deb);
        let path_start = record + MRL3_TEXTURE_PATH_OFFSET;
        bytes[path_start..path_start + path.len()].copy_from_slice(path.as_bytes());
    }

    for (index, hash) in material_hashes.iter().enumerate() {
        let record = material_offset + index * MRL3_MATERIAL_ENTRY_SIZE;
        write_u32(&mut bytes, record, 0x4516_e7ab);
        write_u32(&mut bytes, record + 4, *hash);
        write_u32(&mut bytes, record + 16, 16);
        write_u16(&mut bytes, record + 22, 2);
        write_u64(
            &mut bytes,
            record + 48,
            (resource_offset + index * 16) as u64,
        );
    }
    bytes
}

fn mrl3_paths(bytes: &[u8], count: usize) -> Vec<String> {
    (0..count)
        .map(|index| {
            let start =
                MRL3_HEADER_SIZE + index * MRL3_TEXTURE_ENTRY_SIZE + MRL3_TEXTURE_PATH_OFFSET;
            let field = &bytes[start..start + MRL3_TEXTURE_PATH_CAPACITY];
            let end = field.iter().position(|byte| *byte == 0).expect("path NUL");
            String::from_utf8(field[..end].to_vec()).expect("artificial ASCII path")
        })
        .collect()
}

fn assert_stable_error(error: WeaponBinaryError, expected_code: &str) {
    assert_eq!(error.code(), expected_code);
    let rendered = error.to_string();
    assert!(!rendered.contains("one001"));
    assert!(!rendered.contains("Sensitive"));
    assert!(!rendered.contains("ArtificialWeaponMaterial"));
}

#[test]
fn artificial_pair_preflight_reports_only_bounded_aggregate_facts() {
    let model_pair = pair("one001", "one001");
    let mod3 = artificial_mod3(&["ArtificialWeaponMaterial"]);
    let mrl3 = artificial_mrl3(
        &[r"wp\one\one001\tex\weapon_BM", r"common\texture\shared_BM"],
        &[ARTIFICIAL_MATERIAL_HASH],
    );

    let mod3_report = preflight_mhw_weapon_mod3(&mod3).expect("MOD3 preflight");
    assert_eq!(mod3_report.version(), 237);
    assert_eq!(mod3_report.mesh_count(), 1);
    assert_eq!(mod3_report.material_count(), 1);
    assert_eq!(mod3_report.file_sha256().len(), 64);

    let mrl3_report = preflight_mhw_weapon_mrl3(&mrl3).expect("MRL3 preflight");
    assert_eq!(mrl3_report.version(), 12);
    assert_eq!(mrl3_report.texture_count(), 2);
    assert_eq!(mrl3_report.material_count(), 1);
    assert_eq!(mrl3_report.file_sha256().len(), 64);

    let pair_report = preflight_mhw_weapon_model_pair(&model_pair, &mod3, &mrl3)
        .expect("compatible artificial pair");
    assert_eq!(pair_report.part_role(), WeaponPartRole::Main);
    assert_eq!(pair_report.material_count(), 1);
    assert_eq!(pair_report.material_set_sha256().len(), 64);
    let debug = format!("{pair_report:?}");
    assert!(!debug.contains("one001"));
    assert!(!debug.contains("ArtificialWeaponMaterial"));
}

#[test]
fn mod3_preflight_rejects_header_table_and_buffer_mutations() {
    let valid = artificial_mod3(&["ArtificialWeaponMaterial"]);
    let mut candidates = Vec::new();

    candidates.push(valid[..MOD3_HEADER_SIZE - 1].to_vec());
    let mut wrong_magic = valid.clone();
    wrong_magic[0] ^= 0xff;
    candidates.push(wrong_magic);
    let mut wrong_version = valid.clone();
    write_u16(&mut wrong_version, 4, 236);
    candidates.push(wrong_version);
    let mut excessive_meshes = valid.clone();
    write_u16(&mut excessive_meshes, 8, 8193);
    candidates.push(excessive_meshes);
    let mut bad_material_offset = valid.clone();
    write_u64(&mut bad_material_offset, 64, u64::MAX);
    candidates.push(bad_material_offset);
    let mut unterminated_material = valid.clone();
    unterminated_material[MOD3_HEADER_SIZE..MOD3_HEADER_SIZE + 128].fill(b'A');
    candidates.push(unterminated_material);
    let mut bad_material_id = valid.clone();
    let mesh_offset = MOD3_HEADER_SIZE + MOD3_MATERIAL_ENTRY_SIZE;
    write_u16(&mut bad_material_id, mesh_offset + 6, 1);
    candidates.push(bad_material_id);
    let mut bad_vertex_range = valid.clone();
    write_u64(&mut bad_vertex_range, 24, 35);
    candidates.push(bad_vertex_range);
    let mut bad_face_range = valid.clone();
    write_u32(&mut bad_face_range, mesh_offset + 32, 6);
    candidates.push(bad_face_range);

    for candidate in candidates {
        assert_stable_error(
            preflight_mhw_weapon_mod3(&candidate).expect_err("invalid MOD3"),
            "weapon_binary_format_invalid",
        );
    }
}

#[test]
fn mrl3_preflight_rejects_header_table_resource_and_path_mutations() {
    let valid = artificial_mrl3(
        &[r"wp\one\one001\tex\weapon_BM"],
        &[ARTIFICIAL_MATERIAL_HASH],
    );
    let material_offset = MRL3_HEADER_SIZE + MRL3_TEXTURE_ENTRY_SIZE;
    let mut candidates = Vec::new();

    candidates.push(valid[..MRL3_HEADER_SIZE - 1].to_vec());
    let mut wrong_magic = valid.clone();
    wrong_magic[0] ^= 0xff;
    candidates.push(wrong_magic);
    let mut base_version = valid.clone();
    write_u32(&mut base_version, 4, 11);
    candidates.push(base_version);
    let mut excessive_textures = valid.clone();
    write_u32(&mut excessive_textures, 20, 4097);
    candidates.push(excessive_textures);
    let mut bad_texture_offset = valid.clone();
    write_u64(&mut bad_texture_offset, 24, u64::MAX);
    candidates.push(bad_texture_offset);
    let mut overlapping_materials = valid.clone();
    write_u64(&mut overlapping_materials, 32, MRL3_HEADER_SIZE as u64);
    candidates.push(overlapping_materials);
    let mut bad_texture_id = valid.clone();
    write_u32(&mut bad_texture_id, MRL3_HEADER_SIZE, 0);
    candidates.push(bad_texture_id);
    let mut unterminated_path = valid.clone();
    let path_start = MRL3_HEADER_SIZE + MRL3_TEXTURE_PATH_OFFSET;
    unterminated_path[path_start..path_start + MRL3_TEXTURE_PATH_CAPACITY].fill(b'a');
    candidates.push(unterminated_path);
    let mut odd_resource_count = valid.clone();
    write_u16(&mut odd_resource_count, material_offset + 22, 1);
    candidates.push(odd_resource_count);
    let mut bad_resource_range = valid.clone();
    write_u64(&mut bad_resource_range, material_offset + 48, u64::MAX);
    candidates.push(bad_resource_range);

    for candidate in candidates {
        assert_stable_error(
            preflight_mhw_weapon_mrl3(&candidate).expect_err("invalid MRL3"),
            "weapon_binary_format_invalid",
        );
    }
}

#[test]
fn pair_preflight_requires_exact_jamcrc_material_set_compatibility() {
    let model_pair = pair("one001", "one001");
    let mod3 = artificial_mod3(&["ArtificialWeaponMaterial", "SecondArtificialMaterial"]);
    let matching = artificial_mrl3(
        &[r"wp\one\one001\tex\weapon_BM"],
        &[ARTIFICIAL_MATERIAL_HASH, SECOND_ARTIFICIAL_MATERIAL_HASH],
    );
    assert!(preflight_mhw_weapon_model_pair(&model_pair, &mod3, &matching).is_ok());

    let mismatched = artificial_mrl3(
        &[r"wp\one\one001\tex\weapon_BM"],
        &[ARTIFICIAL_MATERIAL_HASH, 0x1234_5678],
    );
    assert_stable_error(
        preflight_mhw_weapon_model_pair(&model_pair, &mod3, &mismatched)
            .expect_err("mismatched material hash"),
        "weapon_binary_pair_incompatible",
    );
}

#[test]
fn transformer_rewrites_only_exact_source_root_fields_and_is_deterministic() {
    let model_pair = pair("one001", "one001");
    let mod3 = artificial_mod3(&["ArtificialWeaponMaterial"]);
    let paths = [
        r"wp\one\one001\tex\weapon_BM",
        "nativePC/wp/one/one001/tex/one001.tex",
        r"wp\one\one777\tex\one001_shared",
        r"common\texture\shared_BM",
    ];
    let mrl3 = artificial_mrl3(&paths, &[ARTIFICIAL_MATERIAL_HASH]);
    let target = WeaponMainId::parse("one002").expect("target main id");

    let first = transform_mhw_weapon_mrl3_texture_paths(&model_pair, &target, &mod3, &mrl3)
        .expect("transform");
    let second = transform_mhw_weapon_mrl3_texture_paths(&model_pair, &target, &mod3, &mrl3)
        .expect("deterministic transform");
    assert_eq!(first, second);
    assert_eq!(first.bytes().len(), mrl3.len());
    assert_eq!(
        mrl3_paths(first.bytes(), paths.len()),
        vec![
            r"wp\one\one002\tex\weapon_BM",
            "nativePC/wp/one/one002/tex/one002.tex",
            paths[2],
            paths[3],
        ]
    );

    let report = first.report();
    assert_eq!(
        report.transformer_id(),
        MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID
    );
    assert_eq!(
        report.transformer_version(),
        MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_VERSION
    );
    assert_eq!(report.texture_reference_count(), 4);
    assert_eq!(report.rewritten_reference_count(), 2);
    assert_eq!(report.changed_range_count(), 2);
    assert!(report.changed_byte_count() > 0);
    assert_eq!(report.source_sha256().len(), 64);
    assert_eq!(report.output_sha256().len(), 64);
    assert_eq!(report.mapping_sha256().len(), 64);
    let debug = format!("{report:?}");
    assert!(!debug.contains("one001"));
    assert!(!debug.contains("one002"));
    let output_debug = format!("{first:?}");
    assert!(output_debug.contains("byte_len"));
    assert!(!output_debug.contains("[77, 82, 76, 0"));

    let other_target = WeaponMainId::parse("one003").expect("other target main id");
    let other = transform_mhw_weapon_mrl3_texture_paths(&model_pair, &other_target, &mod3, &mrl3)
        .expect("other deterministic mapping");
    assert_ne!(report.mapping_sha256(), other.report().mapping_sha256());

    let allowed_ranges = (0..2)
        .map(|index| {
            let start =
                MRL3_HEADER_SIZE + index * MRL3_TEXTURE_ENTRY_SIZE + MRL3_TEXTURE_PATH_OFFSET;
            start..start + MRL3_TEXTURE_PATH_CAPACITY
        })
        .collect::<Vec<_>>();
    for (index, (before, after)) in mrl3.iter().zip(first.bytes()).enumerate() {
        if before != after {
            assert!(allowed_ranges.iter().any(|range| range.contains(&index)));
        }
    }
}

#[test]
fn generic_registry_bridge_reconstructs_only_the_sealed_artificial_pair() {
    let model_pair = pair("one001", "one001");
    let mod3 = artificial_mod3(&["ArtificialWeaponMaterial"]);
    let mrl3 = artificial_mrl3(
        &[r"wp\one\one001\tex\weapon_BM"],
        &[ARTIFICIAL_MATERIAL_HASH],
    );
    let target = WeaponMainId::parse("one002").expect("target main id");
    let invocation = build_mhw_weapon_mrl3_transform_invocation(&model_pair, &target, &mod3, &mrl3)
        .expect("sealed invocation");
    let dependencies =
        BTreeMap::from([(model_pair.mod3().package_file_id().clone(), mod3.clone())]);

    let bridged = MhwWeaponMrl3TexturePathTransformer
        .transform(ContentTransformRequest::new(
            &invocation,
            model_pair.mrl3().package_file_id(),
            &mrl3,
            &dependencies,
        ))
        .expect("registry bridge transform");
    let direct = transform_mhw_weapon_mrl3_texture_paths(&model_pair, &target, &mod3, &mrl3)
        .expect("direct transform");

    assert_eq!(bridged.bytes(), direct.bytes());
    assert_eq!(
        bridged.canonical_mapping_sha256(),
        direct.report().mapping_sha256()
    );
}

#[test]
fn transformer_maps_all_registered_secondary_parts_to_bs_targets() {
    let fixtures = [
        ("one", "sld"),
        ("sou", "sou_r"),
        ("swo", "saya"),
        ("lan", "sld"),
        ("gun", "sld"),
        ("caxe", "sld"),
    ];
    let mod3 = artificial_mod3(&["ArtificialWeaponMaterial"]);

    for (family, part_prefix) in fixtures {
        let source_main = format!("{family}001");
        let source_part = format!("{part_prefix}001");
        let target_main = format!("bs_{family}002");
        let target_part = format!("bs_{part_prefix}002");
        let model_pair = pair(&source_main, &source_part);
        let source_reference = format!("wp/{family}/{source_main}/tex/{source_part}.tex");
        let mrl3 = artificial_mrl3(&[&source_reference], &[ARTIFICIAL_MATERIAL_HASH]);
        let target = WeaponMainId::parse(&target_main).expect("bs target main id");

        let transformed =
            transform_mhw_weapon_mrl3_texture_paths(&model_pair, &target, &mod3, &mrl3)
                .expect("registered secondary transform");
        assert_eq!(
            mrl3_paths(transformed.bytes(), 1),
            vec![format!("wp/{family}/{target_main}/tex/{target_part}.tex")]
        );
        assert_eq!(transformed.report().rewritten_reference_count(), 1);
    }
}

#[test]
fn transformer_rejects_unsafe_ambiguous_oversized_and_cross_family_mappings() {
    let model_pair = pair("one001", "one001");
    let mod3 = artificial_mod3(&["ArtificialWeaponMaterial"]);
    let target = WeaponMainId::parse("one002").expect("target main id");

    for unsafe_path in [
        r"C:\Users\Sensitive\weapon_BM",
        r"..\outside\weapon_BM",
        r"\server\share\weapon_BM",
        "wp\\one/one001/tex/weapon_BM",
        "wp/one/one001/tex/line\nbreak",
        "wp/one/one001/tex/非ascii",
    ] {
        let mrl3 = artificial_mrl3(&[unsafe_path], &[ARTIFICIAL_MATERIAL_HASH]);
        assert_stable_error(
            transform_mhw_weapon_mrl3_texture_paths(&model_pair, &target, &mod3, &mrl3)
                .expect_err("unsafe reference"),
            "weapon_binary_reference_unsafe",
        );
    }

    /*
     * `one001_variant` 曾被判 ambiguous。#336 改为前缀替换后它是**可安全改写**的——
     * 真实 Mod 的贴图就叫这个形状（`two003_BML`、`bs_two012_XM`），旧规则把它判成歧义
     * 才是缺陷。行为断言见 tests/weapon_reference_rewrite.rs。
     */
    let prefixed = artificial_mrl3(
        &[r"wp\one\one001\tex\one001_variant"],
        &[ARTIFICIAL_MATERIAL_HASH],
    );
    let prefixed = transform_mhw_weapon_mrl3_texture_paths(&model_pair, &target, &mod3, &prefixed)
        .expect("a part-id prefix is safely renameable");
    assert_eq!(
        mrl3_paths(prefixed.bytes(), 1),
        vec![r"wp\one\one002\tex\one002_variant".to_owned()]
    );

    /*
     * 部件 ID 在文件名里出现两次才是真歧义：无法判断作者意图，仍必须报错。
     * 「改不动就降级保留」要等 #336 切片② —— 只有分类器能把源槽位贴图真的留在原路径，
     * 降级结果才是正确的；在此之前降级会静默产出贴图缺失的重定向。
     */
    let ambiguous = artificial_mrl3(
        &[r"wp\one\one001\tex\one001_one001_BM"],
        &[ARTIFICIAL_MATERIAL_HASH],
    );
    assert_stable_error(
        transform_mhw_weapon_mrl3_texture_paths(&model_pair, &target, &mod3, &ambiguous)
            .expect_err("ambiguous reference"),
        "weapon_binary_reference_ambiguous",
    );

    let ambiguous_directory = artificial_mrl3(
        &[r"wp\one\one001\tex\one001\weapon_BM"],
        &[ARTIFICIAL_MATERIAL_HASH],
    );
    assert_stable_error(
        transform_mhw_weapon_mrl3_texture_paths(&model_pair, &target, &mod3, &ambiguous_directory)
            .expect_err("source token in a directory segment is ambiguous"),
        "weapon_binary_reference_ambiguous",
    );

    let prefix = "wp/one/one001/tex/";
    let long_path = format!("{prefix}{}", "a".repeat(254 - prefix.len()));
    let too_long = artificial_mrl3(&[&long_path], &[ARTIFICIAL_MATERIAL_HASH]);
    let bs_target = WeaponMainId::parse("bs_one002").expect("longer target main id");
    assert_stable_error(
        transform_mhw_weapon_mrl3_texture_paths(&model_pair, &bs_target, &mod3, &too_long)
            .expect_err("target path exceeds fixed field"),
        "weapon_binary_path_too_long",
    );

    let cross_family = WeaponMainId::parse("two002").expect("cross-family target");
    let valid = artificial_mrl3(
        &[r"wp\one\one001\tex\weapon_BM"],
        &[ARTIFICIAL_MATERIAL_HASH],
    );
    assert_stable_error(
        transform_mhw_weapon_mrl3_texture_paths(&model_pair, &cross_family, &mod3, &valid)
            .expect_err("cross-family target"),
        "weapon_cross_family_target",
    );
}

#[test]
fn transformer_allows_safe_noop_references_and_preserves_opaque_timestamps() {
    let model_pair = pair("one001", "one001");
    let mut mod3 = artificial_mod3(&["ArtificialWeaponMaterial"]);
    let mut mrl3 = artificial_mrl3(&[r"common\texture\shared_BM"], &[ARTIFICIAL_MATERIAL_HASH]);
    write_u64(&mut mod3, 40, 0x0102_0304_0506_0708);
    write_u64(&mut mrl3, 8, 0x1112_1314_1516_1718);
    let target = WeaponMainId::parse("one002").expect("target main id");

    let transformed = transform_mhw_weapon_mrl3_texture_paths(&model_pair, &target, &mod3, &mrl3)
        .expect("safe shared reference no-op");
    assert_eq!(transformed.bytes(), mrl3);
    assert_eq!(transformed.report().rewritten_reference_count(), 0);
    assert_eq!(transformed.report().changed_range_count(), 0);
    assert_eq!(transformed.report().changed_byte_count(), 0);
    assert_eq!(&transformed.bytes()[8..16], &mrl3[8..16]);
}

#[test]
fn magic_and_version_mutations_always_fail_closed_without_echoing_bytes() {
    let mod3 = artificial_mod3(&["ArtificialWeaponMaterial"]);
    for index in 0..6 {
        let mut mutated = mod3.clone();
        mutated[index] ^= 0x5a;
        assert_stable_error(
            preflight_mhw_weapon_mod3(&mutated).expect_err("mutated MOD3 identity"),
            "weapon_binary_format_invalid",
        );
    }

    let mrl3 = artificial_mrl3(
        &[r"wp\one\one001\tex\weapon_BM"],
        &[ARTIFICIAL_MATERIAL_HASH],
    );
    for index in 0..8 {
        let mut mutated = mrl3.clone();
        mutated[index] ^= 0xa5;
        assert_stable_error(
            preflight_mhw_weapon_mrl3(&mutated).expect_err("mutated MRL3 identity"),
            "weapon_binary_format_invalid",
        );
    }
}
