//! 冻结会透传到前端的武器稳定错误码集合。
//!
//! `src-tauri/src/replacement_commands.rs` 的 `analysis_error_to_command_error`
//! 把 `ReplacementAdapterError::AnalysisRejected { code }` 原样返回给前端，
//! 因此这些码是对 UI 的公开契约。前端在
//! `src/features/replacements/replacementErrorText.ts` 里用同一份集合做
//! `Record<WeaponReplacementErrorCode, ...>` 的穷尽映射。
//!
//! 下面两个 `match` 是编译期闸门：新增枚举变体会直接编译失败；
//! `EXPECTED_*_CODES` 断言则保证码字符串本身不被悄悄改名。
//!
//! 注意本文件只管 Rust 这一侧：即使这里全绿，前端也可能没补对应文案，
//! 那时用户会退回无信息量的兜底提示。跨语言的集合相等断言在
//! `src/features/replacements/replacementErrorCodeContract.test.mjs`——
//! 它直接解析本 crate 的 `code()` 并与前端码表比对，改动任一侧都会让它先红。

use hmm_games_mhw::{WeaponAnalysisError, WeaponBinaryError};

/// 穷尽匹配；新增变体在此处编译失败。
fn analysis_code(error: WeaponAnalysisError) -> &'static str {
    match error {
        WeaponAnalysisError::InvalidPackageFileId
        | WeaponAnalysisError::DuplicatePackageFileId
        | WeaponAnalysisError::UnsafePath
        | WeaponAnalysisError::DuplicateAssetPath
        | WeaponAnalysisError::CaseInsensitivePathCollision
        | WeaponAnalysisError::SourceNotFound
        | WeaponAnalysisError::MultipleSourceRoots
        | WeaponAnalysisError::MixedFamily
        | WeaponAnalysisError::UnknownFamily
        | WeaponAnalysisError::InvalidMainId
        | WeaponAnalysisError::UnknownPart
        | WeaponAnalysisError::IncompleteBinaryPair
        | WeaponAnalysisError::MixedInstallPayload
        | WeaponAnalysisError::UnsupportedResource
        | WeaponAnalysisError::IdentityInvalid => error.code(),
    }
}

/// 穷尽匹配；新增变体在此处编译失败。
fn binary_code(error: WeaponBinaryError) -> &'static str {
    match error {
        WeaponBinaryError::FormatInvalid
        | WeaponBinaryError::PairIncompatible
        | WeaponBinaryError::ReferenceUnsafe
        | WeaponBinaryError::ReferenceAmbiguous
        | WeaponBinaryError::PathTooLong
        | WeaponBinaryError::CrossFamilyTarget
        | WeaponBinaryError::OutputInvalid => error.code(),
    }
}

const ALL_ANALYSIS_ERRORS: [WeaponAnalysisError; 15] = [
    WeaponAnalysisError::InvalidPackageFileId,
    WeaponAnalysisError::DuplicatePackageFileId,
    WeaponAnalysisError::UnsafePath,
    WeaponAnalysisError::DuplicateAssetPath,
    WeaponAnalysisError::CaseInsensitivePathCollision,
    WeaponAnalysisError::SourceNotFound,
    WeaponAnalysisError::MultipleSourceRoots,
    WeaponAnalysisError::MixedFamily,
    WeaponAnalysisError::UnknownFamily,
    WeaponAnalysisError::InvalidMainId,
    WeaponAnalysisError::UnknownPart,
    WeaponAnalysisError::IncompleteBinaryPair,
    WeaponAnalysisError::MixedInstallPayload,
    WeaponAnalysisError::UnsupportedResource,
    WeaponAnalysisError::IdentityInvalid,
];

const ALL_BINARY_ERRORS: [WeaponBinaryError; 7] = [
    WeaponBinaryError::FormatInvalid,
    WeaponBinaryError::PairIncompatible,
    WeaponBinaryError::ReferenceUnsafe,
    WeaponBinaryError::ReferenceAmbiguous,
    WeaponBinaryError::PathTooLong,
    WeaponBinaryError::CrossFamilyTarget,
    WeaponBinaryError::OutputInvalid,
];

const EXPECTED_ANALYSIS_CODES: [&str; 15] = [
    "weapon_invalid_package_file_id",
    "weapon_duplicate_package_file_id",
    "weapon_unsafe_path",
    "weapon_duplicate_asset_path",
    "weapon_case_insensitive_path_collision",
    "weapon_source_not_found",
    "weapon_multiple_source_roots",
    "weapon_mixed_family",
    "weapon_unknown_family",
    "weapon_invalid_main_id",
    "weapon_unknown_part",
    "weapon_incomplete_binary_pair",
    "weapon_mixed_install_payload",
    "weapon_unsupported_resource",
    "weapon_identity_invalid",
];

const EXPECTED_BINARY_CODES: [&str; 7] = [
    "weapon_binary_format_invalid",
    "weapon_binary_pair_incompatible",
    "weapon_binary_reference_unsafe",
    "weapon_binary_reference_ambiguous",
    "weapon_binary_path_too_long",
    "weapon_cross_family_target",
    "weapon_transformer_output_invalid",
];

#[test]
fn weapon_analysis_error_codes_are_frozen_ui_contract() {
    let actual: Vec<&str> = ALL_ANALYSIS_ERRORS.into_iter().map(analysis_code).collect();
    assert_eq!(actual, EXPECTED_ANALYSIS_CODES.to_vec());
}

#[test]
fn weapon_binary_error_codes_are_frozen_ui_contract() {
    let actual: Vec<&str> = ALL_BINARY_ERRORS.into_iter().map(binary_code).collect();
    assert_eq!(actual, EXPECTED_BINARY_CODES.to_vec());
}

#[test]
fn weapon_error_codes_do_not_collide_across_enums() {
    let mut all: Vec<&str> = EXPECTED_ANALYSIS_CODES
        .into_iter()
        .chain(EXPECTED_BINARY_CODES)
        .collect();
    let total = all.len();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), total, "前端按码做穷尽映射，码必须全局唯一");
}
