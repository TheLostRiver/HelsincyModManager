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

/// MOD3 预检**必须**继续关闭的那些门：身份（magic / version）、计数上限、区间落在
/// 文件内、区间不重叠、offset 单调，以及界定「进材质哈希的字节」的那组字符串约束。
///
/// `#346` 按类删掉的是对**不参与写入、不参与哈希**的字段的语义猜测（见 `binary.rs`
/// 里那段说明）。这里逐条列出的都不属于那一类：MOD3 虽然逐字节复制，但这些边界保证
/// 的是**解析本身**不越界、以及材质哈希取自确定的字节。
///
/// 逐个具名而不是塞进一个匿名数组：原先写成 `for candidate in candidates` 的循环，
/// 任何一条转红都只报「invalid MOD3」，看不出是哪一条——反向验证时那等于没有信号。
#[test]
fn mod3_preflight_still_rejects_real_boundary_violations() {
    let valid = artificial_mod3(&["ArtificialWeaponMaterial"]);
    let material_offset = MOD3_HEADER_SIZE;
    let mesh_offset = material_offset + MOD3_MATERIAL_ENTRY_SIZE;

    let mut cases: Vec<(&str, Vec<u8>)> = Vec::new();

    // 文件信封：头部都装不下。
    cases.push(("头部被截断", valid[..MOD3_HEADER_SIZE - 1].to_vec()));

    // 身份确认。
    let mut wrong_magic = valid.clone();
    wrong_magic[0] ^= 0xff;
    cases.push(("magic 不符", wrong_magic));
    let mut wrong_version = valid.clone();
    write_u16(&mut wrong_version, 4, 236);
    cases.push(("version 不符（非 Iceborne）", wrong_version));

    // 计数上限。
    let mut excessive_meshes = valid.clone();
    write_u16(&mut excessive_meshes, 8, 8193);
    cases.push(("网格数超上限", excessive_meshes));
    let mut zero_materials = valid.clone();
    write_u16(&mut zero_materials, 10, 0);
    cases.push(("材质数为 0（取不到材质哈希）", zero_materials));

    // 区间必须落在文件内。
    let mut bad_material_offset = valid.clone();
    write_u64(&mut bad_material_offset, 64, u64::MAX);
    cases.push(("材质表 offset 越界", bad_material_offset));
    let mut mesh_table_past_end = valid.clone();
    write_u16(&mut mesh_table_past_end, 8, 64);
    cases.push(("网格表长度超出文件尾", mesh_table_past_end));
    let mut vertex_buffer_past_end = valid.clone();
    write_u64(&mut vertex_buffer_past_end, 24, u64::MAX);
    cases.push(("顶点缓冲区长度越界", vertex_buffer_past_end));

    // 区间不得重叠：把网格表挪到材质表头上。
    let mut overlapping_tables = valid.clone();
    write_u64(&mut overlapping_tables, 72, material_offset as u64);
    cases.push(("网格表与材质表重叠", overlapping_tables));

    // offset 必须单调递增。
    let mut non_monotonic = valid.clone();
    write_u64(&mut non_monotonic, 72, (mesh_offset - 8) as u64);
    write_u64(&mut non_monotonic, 64, mesh_offset as u64);
    cases.push(("offset 不再单调", non_monotonic));

    // 材质名的字节约束：它们界定了进 `material_set_sha256` 的字节，是承重的。
    let mut unterminated_material = valid.clone();
    unterminated_material[material_offset..material_offset + MOD3_MATERIAL_ENTRY_SIZE].fill(b'A');
    cases.push(("材质名没有 NUL 终止", unterminated_material));
    let mut empty_material_name = valid.clone();
    empty_material_name[material_offset..material_offset + MOD3_MATERIAL_ENTRY_SIZE].fill(0);
    cases.push(("材质名为空", empty_material_name));
    let mut control_character_material = valid.clone();
    control_character_material[material_offset + 1] = 0x07;
    cases.push(("材质名含控制字符", control_character_material));
    let mut trailing_space_material = valid.clone();
    trailing_space_material[material_offset] = b' ';
    cases.push(("材质名以空格开头", trailing_space_material));

    for (label, candidate) in cases {
        let error = preflight_mhw_weapon_mod3(&candidate)
            .map(|report| format!("{report:?}"))
            .expect_err(&format!("「{label}」必须被拒"));
        assert_stable_error(error, "weapon_binary_format_invalid");
    }
}

// `#346`：真实 Iceborne 文件里存在的形态**必须**通过预检。下面四条此前都会被判
// 「格式无法识别，可能不是 Iceborne 版本或文件已损坏」——而包本身完好，文案还引导
// 玩家去重新下载。它们都是对不参与写入、不参与哈希的字段做的语义猜测。
//
// 分成四个独立用例而不是一个函数里跑四遍：反向验证要能指出**恰好**是哪一条依赖
// 哪一条被删的检查，塞在一起的话第一条转红就把后面全挡住了。

const MOD3_MESH_TABLE_OFFSET: usize = MOD3_HEADER_SIZE + MOD3_MATERIAL_ENTRY_SIZE;

/// 两个网格共用同一材质：`bow017.mod3` 的 9 条材质里 `Ch_Wp_Mt__1` 出现两次，去重后
/// 8 条，与配对的 `bow017.mrl3` 的 8 条恰好对上——去重才是对的口径。
#[test]
fn mod3_preflight_accepts_a_duplicated_material_name() {
    let duplicated = artificial_mod3(&["ArtificialWeaponMaterial", "ArtificialWeaponMaterial"]);
    let duplicated_report =
        preflight_mhw_weapon_mod3(&duplicated).expect("材质重名不是损坏信号，必须通过");
    let single_report = preflight_mhw_weapon_mod3(&artificial_mod3(&["ArtificialWeaponMaterial"]))
        .expect("单份材质必须通过");

    assert_eq!(
        duplicated_report.material_set_sha256(),
        single_report.material_set_sha256(),
        "去重后的材质集合相同，摘要必须相同——否则 facts 会随重名条数漂移"
    );
    assert_eq!(
        duplicated_report.material_count(),
        2,
        "报告里的 material_count 是 header 声明值，不是去重后的条数"
    );
}

/// offset 60 保留原始模型的绝对索引：`ya017.mod3` 是 1203 + 196 > 196（`vertexCount`）。
/// 这个字段的语义未经证实，不参与判定。
#[test]
fn mod3_preflight_accepts_an_absolute_vertex_index_at_offset_sixty() {
    let mut candidate = artificial_mod3(&["ArtificialWeaponMaterial"]);
    write_u32(&mut candidate, MOD3_MESH_TABLE_OFFSET + 60, 1203);

    preflight_mhw_weapon_mod3(&candidate).expect("offset 60 的绝对索引不是损坏信号，必须通过");
}

/// 网格引用的 `materialId` 超出 `materialCount`：我们从不按网格索引材质。
#[test]
fn mod3_preflight_accepts_an_out_of_range_mesh_material_id() {
    let mut candidate = artificial_mod3(&["ArtificialWeaponMaterial"]);
    write_u16(&mut candidate, MOD3_MESH_TABLE_OFFSET + 6, 9);

    preflight_mhw_weapon_mod3(&candidate).expect("materialId 不参与判定");
}

/// 网格的面数不是 3 的倍数、顶点数/块大小为零：同样是对不参与写入的字段的猜测。
#[test]
fn mod3_preflight_accepts_unguessable_mesh_counts() {
    let mut odd_face_count = artificial_mod3(&["ArtificialWeaponMaterial"]);
    write_u32(&mut odd_face_count, MOD3_MESH_TABLE_OFFSET + 32, 7);
    preflight_mhw_weapon_mod3(&odd_face_count).expect("meshFaceCount 不参与判定");

    let mut zero_valued = artificial_mod3(&["ArtificialWeaponMaterial"]);
    write_u16(&mut zero_valued, MOD3_MESH_TABLE_OFFSET + 2, 0);
    write_u32(&mut zero_valued, MOD3_MESH_TABLE_OFFSET + 32, 0);
    zero_valued[MOD3_MESH_TABLE_OFFSET + 14] = 0;
    preflight_mhw_weapon_mod3(&zero_valued).expect("网格零值不参与判定");
}

/// 由网格字段**推算**出的顶点缓冲区末尾越过 `vertexBufferSize`：这条上限是用同一族
/// 未证实的字段语义（`+16` / `+36` / `+14`）算出来的，真实数据里还没撞上反例——但
/// 语义既然没被证实，留着就是定时炸弹。这里把它明确钉成「不参与判定」。
///
/// 只动 `+36`（那组算术里的 vertexBase），不碰面数相关字段，因此这条用例**只**依赖
/// 推算式上限这一条被删的检查；反向验证时它与网格计数那条互不干扰。
#[test]
fn mod3_preflight_accepts_a_derived_vertex_end_past_the_buffer() {
    let mut candidate = artificial_mod3(&["ArtificialWeaponMaterial"]);
    write_u32(&mut candidate, MOD3_MESH_TABLE_OFFSET + 36, 4096);

    preflight_mhw_weapon_mod3(&candidate).expect("推算出的 vertexEnd 不参与判定");
}

/// 材质重名的 MOD3 仍要能与 MRL3 配对——`#346` 的实际卡点在这里：只放宽 MOD3 预检
/// 而配对比对对不上，弓包依旧不能重定向，只是错误码从 `format_invalid` 换成
/// `pair_incompatible`。
#[test]
fn a_mod3_with_a_duplicated_material_name_still_pairs_with_its_mrl3() {
    let model_pair = pair("one001", "one001");
    let mod3 = artificial_mod3(&["ArtificialWeaponMaterial", "ArtificialWeaponMaterial"]);
    let mrl3 = artificial_mrl3(
        &[r"wp\one\one001\tex\weapon_BM"],
        &[ARTIFICIAL_MATERIAL_HASH],
    );

    let report = preflight_mhw_weapon_model_pair(&model_pair, &mod3, &mrl3)
        .expect("MOD3 去重后的材质集合必须与 MRL3 的一条材质对上");

    assert_eq!(
        report.material_count(),
        1,
        "配对报告的材质数取自 MRL3——它才是被改写的那一侧"
    );
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

/// `#346`：配对由**结构**建立（同族根、同部件 ID、一个 MOD3 配一个 MRL3），两侧的
/// 材质集合**不必**逐条对上。
///
/// 原先这里断言「材质哈希不完全相等即 `pair_incompatible`」。那是同一病根的第六处：
/// 它用 MOD3 的材质集合去关联 MRL3，而改写链路根本不读这个关联——`mrl3_transform`
/// 两次调用 `parse_model_pair` 都把 `ParsedMod3` 丢掉，只比对 **MRL3 自己**的材质哈希
/// 前后一致。
///
/// 真实数据也证伪了它：弓包的 `ya017.mod3` 有一条名为 `Shadow_invalid_Mt__1` 的失效
/// 材质槽，与配对的 `ya017.mrl3` 在那一位存的哈希不同，而文件本身可用（安装正常、
/// 游戏内正常）。对照组 `bow017` / `swo035` / `saya035ol` 三对是完全相等的——相等是
/// 常态，但不是格式约束。
#[test]
fn pair_preflight_accepts_a_divergent_material_set() {
    let model_pair = pair("one001", "one001");
    let mod3 = artificial_mod3(&["ArtificialWeaponMaterial", "SecondArtificialMaterial"]);

    // 常态：两侧完全相等。
    let matching = artificial_mrl3(
        &[r"wp\one\one001\tex\weapon_BM"],
        &[ARTIFICIAL_MATERIAL_HASH, SECOND_ARTIFICIAL_MATERIAL_HASH],
    );
    preflight_mhw_weapon_model_pair(&model_pair, &mod3, &matching).expect("两侧相等必须通过");

    // `ya017` 的形态：一条对得上、一条对不上。
    let partially_divergent = artificial_mrl3(
        &[r"wp\one\one001\tex\weapon_BM"],
        &[ARTIFICIAL_MATERIAL_HASH, 0x1234_5678],
    );
    let report = preflight_mhw_weapon_model_pair(&model_pair, &mod3, &partially_divergent)
        .expect("材质集合部分不一致不是损坏信号");
    assert_eq!(
        report.material_count(),
        2,
        "配对报告的材质数取自 MRL3——它才是被改写的那一侧"
    );

    // 完全不相交也不否决：MOD3 的材质集合根本不参与改写，不该由它否决整对。
    let fully_divergent = artificial_mrl3(
        &[r"wp\one\one001\tex\weapon_BM"],
        &[0x1234_5678, 0x9abc_def0],
    );
    preflight_mhw_weapon_model_pair(&model_pair, &mod3, &fully_divergent)
        .expect("MOD3 的材质集合不参与改写");
}

/// 但**结构**不配对仍然否决——那才是 `pair_incompatible` 承重的地方。
///
/// 这段判定断言的是 `WeaponModelPair` 的内部不变量（`analyze_mhw_weapon_assets` 只产出
/// 结构正确的对），所以从公开 API 造不出反例。这里用 MRL3 侧的改写链路间接确认它仍在
/// 位：跨族目标必须被 `weapon_cross_family_target` 挡下，而不是悄悄改写成别的族。
#[test]
fn pair_transform_still_refuses_a_cross_family_target() {
    let model_pair = pair("one001", "one001");
    let mod3 = artificial_mod3(&["ArtificialWeaponMaterial"]);
    let mrl3 = artificial_mrl3(
        &[r"wp\one\one001\tex\weapon_BM"],
        &[ARTIFICIAL_MATERIAL_HASH],
    );
    let cross_family = WeaponMainId::parse("two002").expect("cross-family target");

    assert_stable_error(
        transform_mhw_weapon_mrl3_texture_paths(&model_pair, &cross_family, &mod3, &mrl3)
            .expect_err("跨族目标必须被拒"),
        "weapon_cross_family_target",
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
