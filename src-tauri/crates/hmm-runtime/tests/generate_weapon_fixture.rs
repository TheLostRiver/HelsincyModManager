//! 武器重定向真机 fixture 生成器。
//!
//! 常规 `cargo test` 下本测试直接返回，不产出任何东西、不影响 CI。
//! 只有在显式给出输出目录时才生成：
//!
//! ```text
//! HMM_FIXTURE_OUT_DIR=D:/DEV/HMM-WR05-fixture \
//!   cargo test -p hmm-runtime --test generate_weapon_fixture
//! ```
//!
//! 生成三份东西：
//! - `mhw-minimal/`：人工游戏目录，含目标武器 one002 的原版 MOD3/MRL3。
//! - `weapon-mod-one001/`：待导入的 Mod 源目录，含源武器 one001 的 MOD3/MRL3，
//!   **以及 readme.txt 与 preview/ 预览图**——真实 Mod 必然携带杂项文件，
//!   这正是 WR 真机不可用修复的验证点，不要为了"干净"把它删掉。
//! - 两个可直接导入的 zip：`-flat.zip`（nativePC 在根）与 `-wrapped.zip`
//!   （外层包一层 `weapon-mod-one001/`），用于对比验证包根目录剥离。
//!
//! 二进制布局与 `weapon_transform_lifecycle.rs` 的人工固件一致，确保
//! `parse_model_pair` 能解析、mrl3 贴图路径 transform 能执行。

use hmm_core::{
    GameId, ModId, PackageFileId, ProfileId, ReplacementBinding, ReplacementBindingId,
    ReplacementTargetId,
};
use hmm_games_mhw::{
    generate_mhw_equipment_stable_id, EquipmentCandidateTargetKind, MhwReplacementAdapter,
};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAdapterResult,
    ReplacementAnalysisRequest, ReplacementAsset, ReplacementAssetContentReader,
    RetargetPlanRequest,
};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

const MOD3_HEADER_SIZE: usize = 320;
const MOD3_MATERIAL_ENTRY_SIZE: usize = 128;
const MOD3_MESH_ENTRY_SIZE: usize = 80;
const MRL3_HEADER_SIZE: usize = 40;
const MRL3_TEXTURE_ENTRY_SIZE: usize = 272;
const MRL3_MATERIAL_ENTRY_SIZE: usize = 56;
const ARTIFICIAL_MATERIAL_HASH: u32 = 0xa7f6_8bf8;

/// 外层包目录名。真机里绝大多数 Mod 压缩包都带这么一层作者自建目录。
const WRAP: &str = "weapon-mod-one001";
const MOD3: &str = "nativePC/wp/one/one001/mod/one001.mod3";
const MRL3: &str = "nativePC/wp/one/one001/mod/one001.mrl3";
/// 真实 Mod 必然携带的杂项文件。留着它们，不要为了"干净"删掉——
/// 这正是这次修复（杂项从拒绝改为忽略）的验证点。
const README: &str = "readme.txt";
const PREVIEW: &str = "preview/preview.png";

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn artificial_mod3(material_name: &str) -> Vec<u8> {
    let material_offset = MOD3_HEADER_SIZE;
    let mesh_offset = material_offset + MOD3_MATERIAL_ENTRY_SIZE;
    let vertex_offset = mesh_offset + MOD3_MESH_ENTRY_SIZE + 4;
    let vertex_buffer_size = 36usize;
    let face_offset = vertex_offset + vertex_buffer_size;
    let face_buffer_size = 8usize;
    let vertex_remap_offset = face_offset + face_buffer_size;
    let mut bytes = vec![0u8; vertex_remap_offset + 24];
    write_u32(&mut bytes, 0, 0x0044_4f4d);
    write_u16(&mut bytes, 4, 237);
    write_u16(&mut bytes, 8, 1);
    write_u16(&mut bytes, 10, 1);
    write_u32(&mut bytes, 12, 3);
    write_u32(&mut bytes, 16, 3);
    // offset 24 是 vertex_buffer_size，不是 material_offset——
    // parse_mod3 在此处读顶点缓冲区长度，写错会让后续偏移全部错位。
    write_u64(&mut bytes, 24, vertex_buffer_size as u64);
    write_u64(&mut bytes, 64, material_offset as u64);
    write_u64(&mut bytes, 72, mesh_offset as u64);
    write_u64(&mut bytes, 80, vertex_offset as u64);
    write_u64(&mut bytes, 88, face_offset as u64);
    write_u64(&mut bytes, 96, vertex_remap_offset as u64);
    let material = material_name.as_bytes();
    bytes[material_offset..material_offset + material.len()].copy_from_slice(material);
    write_u16(&mut bytes, mesh_offset + 2, 3);
    write_u16(&mut bytes, mesh_offset + 6, 0);
    write_u16(&mut bytes, mesh_offset + 8, 1);
    bytes[mesh_offset + 14] = 12;
    write_u32(&mut bytes, mesh_offset + 32, 3);
    write_u32(&mut bytes, vertex_remap_offset, 4);
    bytes
}

fn artificial_mrl3(texture_path: &str) -> Vec<u8> {
    let texture_offset = MRL3_HEADER_SIZE;
    let material_offset = texture_offset + MRL3_TEXTURE_ENTRY_SIZE;
    let material_end = material_offset + MRL3_MATERIAL_ENTRY_SIZE;
    let resource_offset = (material_end + 15) & !15;
    let mut bytes = vec![0u8; resource_offset + 16];
    write_u32(&mut bytes, 0, 0x004c_524d);
    write_u32(&mut bytes, 4, 12);
    write_u32(&mut bytes, 16, 1);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, texture_offset as u64);
    write_u64(&mut bytes, 32, material_offset as u64);
    write_u32(&mut bytes, texture_offset, 0x241f_5deb);
    let path = texture_path.as_bytes();
    bytes[texture_offset + 16..texture_offset + 16 + path.len()].copy_from_slice(path);
    write_u32(&mut bytes, material_offset, 0x4516_e7ab);
    write_u32(&mut bytes, material_offset + 4, ARTIFICIAL_MATERIAL_HASH);
    write_u32(&mut bytes, material_offset + 16, 16);
    write_u16(&mut bytes, material_offset + 22, 2);
    write_u64(&mut bytes, material_offset + 48, resource_offset as u64);
    bytes
}

fn write_file(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create fixture parent dir");
    }
    fs::write(&path, bytes).expect("write fixture file");
    println!("  {} ({} bytes)", relative, bytes.len());
}

fn write_zip(path: &Path, entries: &[(&str, Vec<u8>)]) {
    let file = fs::File::create(path).expect("create zip file");
    let mut archive = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in entries {
        archive.start_file(*name, options).expect("start zip entry");
        archive.write_all(bytes).expect("write zip entry");
    }
    archive.finish().expect("finish zip archive");
    println!(
        "  {} ({} entries, {} bytes)",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
        entries.len(),
        fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    );
}

/// 本文件里的两个测试并行执行，而校验测试要读生成测试的产物。
/// 用一把进程内锁 + 幂等写入，谁先到谁生成，避免先删目录后出现的竞态。
static EMIT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn ensure_fixture(out: &Path) -> PathBuf {
    let _guard = EMIT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    emit_fixture(out);
    out.join(WRAP)
}

fn emit_fixture(out: &Path) {
    fs::create_dir_all(out).expect("create fixture output dir");

    // ---- 人工游戏目录：目标武器 one002 的原版资源 ----
    let game = out.join("mhw-minimal");
    println!("人工游戏目录 -> {}", game.display());
    write_file(
        &game,
        "MonsterHunterWorld.exe",
        b"artificial-mhw-executable-placeholder\n",
    );
    write_file(
        &game,
        "nativePC/wp/one/one002/mod/one002.mod3",
        &artificial_mod3("ArtificialWeaponMaterial"),
    );
    write_file(
        &game,
        "nativePC/wp/one/one002/mod/one002.mrl3",
        &artificial_mrl3("wp\\one\\one002\\tex\\weapon_BM"),
    );
    // ---- 前置桩：文件存在但签名必然与 mhw-prerequisites.default.json 不符 ----
    // 完全缺失会给出 MissingRequiredFile → 前置决策 Blocked，真机验收走不下去；
    // 存在但签名不符是 InstalledUnverified → Warning，不阻塞——这正是真实玩家
    // （loader 版本与规则库签名不一致）最常见的前置状态。
    write_file(&game, "dinput8.dll", b"stub-dinput8\n");
    write_file(&game, "loader.dll", b"stub-loader\n");
    write_file(
        &game,
        "loader-config.json",
        br#"{"enablePluginLoader":true}"#,
    );
    write_file(
        &game,
        "nativePC/plugins/MonsterLoader.dll",
        b"stub-monster-loader\n",
    );
    write_file(
        &game,
        "nativePC/plugins/QuestLoader.dll",
        b"stub-quest-loader\n",
    );
    write_file(
        &game,
        "nativePC/plugins/!CRCBypass.dll",
        b"stub-crc-bypass\n",
    );

    // ---- Mod 源目录：源武器 one001，并刻意携带真实 Mod 必然有的杂项文件 ----
    let package = out.join(WRAP);
    println!("Mod 源目录 -> {}", package.display());
    let payload: Vec<(&str, Vec<u8>)> = vec![
        (MOD3, artificial_mod3("ArtificialWeaponMaterial")),
        (MRL3, artificial_mrl3("wp\\one\\one001\\tex\\weapon_BM")),
        (
            README,
            b"Artificial weapon fixture for WR-05 verification.\nSource: one001 -> target: one002.\n"
                .to_vec(),
        ),
        (
            PREVIEW,
            b"\x89PNG\r\n\x1a\n artificial preview placeholder\n".to_vec(),
        ),
    ];
    for (relative, bytes) in &payload {
        write_file(&package, relative, bytes);
    }

    // ---- 两个可直接导入的 zip，用于对比验证包根目录剥离 ----
    println!("可导入 zip ->");
    write_zip(&out.join(format!("{WRAP}-flat.zip")), &payload);
    let wrapped_names: Vec<String> = payload
        .iter()
        .map(|(relative, _)| format!("{WRAP}/{relative}"))
        .collect();
    let wrapped: Vec<(&str, Vec<u8>)> = wrapped_names
        .iter()
        .zip(&payload)
        .map(|(name, (_, bytes))| (name.as_str(), bytes.clone()))
        .collect();
    write_zip(&out.join(format!("{WRAP}-wrapped.zip")), &wrapped);

    println!("fixture 生成完毕：{}", out.display());
}

#[test]
fn generate_weapon_retarget_fixture() {
    let Ok(out) = std::env::var("HMM_FIXTURE_OUT_DIR") else {
        println!("HMM_FIXTURE_OUT_DIR 未设置，跳过 fixture 生成。");
        return;
    };
    ensure_fixture(Path::new(&out));
}

struct FixtureContentReader {
    root: PathBuf,
}

impl ReplacementAssetContentReader for FixtureContentReader {
    fn read_asset_content(
        &self,
        package_file_id: &PackageFileId,
        max_bytes: u64,
    ) -> ReplacementAdapterResult<Vec<u8>> {
        let path = self.root.join(package_file_id.as_str());
        let bytes =
            fs::read(&path).map_err(|_| ReplacementAdapterError::SourceContentUnavailable)?;
        if bytes.len() as u64 > max_bytes {
            return Err(ReplacementAdapterError::SourceContentUnavailable);
        }
        Ok(bytes)
    }
}

/// 拿刚生成的 fixture 真跑一遍 adapter 全链路，确认固件有效——
/// 避免用户兴冲冲导入后才发现二进制根本解析不了。
#[test]
fn weapon_retarget_fixture_survives_analysis_and_plan() {
    let Ok(out) = std::env::var("HMM_FIXTURE_OUT_DIR") else {
        println!("HMM_FIXTURE_OUT_DIR 未设置，跳过 fixture 校验。");
        return;
    };
    let out = PathBuf::from(out);
    let package = ensure_fixture(&out);
    assert!(
        package.is_dir(),
        "fixture 目录未生成：{}",
        package.display()
    );

    // 刻意用"带外层目录 + 携带 readme/预览图"的形态，这正是真机最典型的包，
    // 也是这次修复要覆盖的场景。
    let assets = vec![
        ReplacementAsset::new(PackageFileId::new(README), format!("{WRAP}/{README}")),
        ReplacementAsset::new(PackageFileId::new(PREVIEW), format!("{WRAP}/{PREVIEW}")),
        ReplacementAsset::new(PackageFileId::new(MOD3), format!("{WRAP}/{MOD3}")),
        ReplacementAsset::new(PackageFileId::new(MRL3), format!("{WRAP}/{MRL3}")),
    ];

    let analysis = MhwReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets.clone(),
        })
        .expect("wrapped package with extras must analyze");
    let source = analysis.single_source().expect("single weapon source");
    assert_eq!(source.source_type().as_str(), "weapon");

    let target_id = generate_mhw_equipment_stable_id(
        EquipmentCandidateTargetKind::Weapon,
        "wp/one",
        "nativePC/wp/one/one002",
    )
    .expect("target stable id");
    let binding = ReplacementBinding::new(
        ReplacementBindingId::parse("binding-fixture").expect("binding id"),
        ModId::new("fixture-weapon-mod"),
        ProfileId::new("default"),
        source.id().clone(),
        ReplacementTargetId::parse(&target_id).expect("target id"),
        1,
    )
    .expect("fixture binding");

    let plan = MhwReplacementAdapter
        .build_retarget_plan_with_content(
            RetargetPlanRequest {
                game_id: GameId::mhw(),
                binding,
                assets,
            },
            &FixtureContentReader {
                root: package.clone(),
            },
        )
        .expect("retarget plan from real fixture bytes");

    assert_eq!(plan.actions().len(), 2, "MOD3 与 MRL3 各一条 action");

    // flat 形态（nativePC 直接位于包根）是另一类常见包，同样必须走通。
    let flat_assets: Vec<ReplacementAsset> = [README, PREVIEW, MOD3, MRL3]
        .into_iter()
        .map(|path| ReplacementAsset::new(PackageFileId::new(path), path))
        .collect();
    let flat_analysis = MhwReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: flat_assets,
        })
        .expect("flat package must analyze");
    assert_eq!(
        flat_analysis.single_source().expect("flat source").id(),
        source.id(),
        "两种包形态必须解析出同一个 source"
    );

    for zip_name in [format!("{WRAP}-flat.zip"), format!("{WRAP}-wrapped.zip")] {
        assert!(out.join(&zip_name).is_file(), "缺少可导入 zip：{zip_name}");
    }

    println!(
        "fixture 全链路通过：source={} target=one002 actions={}",
        source.id().as_str(),
        plan.actions().len()
    );
}
