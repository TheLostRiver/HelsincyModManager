import type { ReplacementCopy } from "./replacementCopy";

/**
 * 替换目标（Armor / Weapon retarget）错误码定义与取词入口。
 *
 * 后端把稳定错误码原样透传给前端（src-tauri/src/replacement_commands.rs 的
 * analysis_error_to_command_error 对 AnalysisRejected { code } 直接返回 code），
 * 因此前端必须覆盖 hmm-games-mhw 侧 WeaponAnalysisError / WeaponBinaryError 的全部码，
 * 否则用户只会看到无信息量的兜底文案。
 *
 * 三层闸门共同防止新码悄悄退回兜底提示：
 * 1. replacementCopy.ts 的 Record<WeaponReplacementErrorCode, ErrorCopy>（每种语言各一份）
 *    —— 码表里有码却没写文案时 tsc 失败。
 * 2. hmm-games-mhw/tests/weapon_error_code_contract.rs —— Rust 枚举新增变体时编译失败。
 * 3. replacementErrorCodeContract.test.mjs —— 跨语言比对 Rust `code()` 与本文件码表的集合，
 *    并检查 replacement_commands.rs 吐出的通用码都有文案。
 *    只有第 3 层能挡住"补了 Rust 却没补前端文案"，前两层各自只管本语言内部。
 *
 * 文案约束沿用 docs/WEAPON_RETARGET_DESIGN.md 的脱敏要求：
 * 只出现稳定码、聚合描述与可执行建议，不回显路径、offset、material 名或二进制内容。
 */

export const WEAPON_REPLACEMENT_ERROR_CODES = [
  // WeaponAnalysisError
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
  // WeaponBinaryError
  "weapon_binary_format_invalid",
  "weapon_binary_pair_incompatible",
  "weapon_binary_reference_unsafe",
  "weapon_binary_reference_ambiguous",
  "weapon_binary_path_too_long",
  "weapon_cross_family_target",
  "weapon_transformer_output_invalid",
] as const;

export type WeaponReplacementErrorCode = (typeof WEAPON_REPLACEMENT_ERROR_CODES)[number];

const weaponErrorCodeSet = new Set<string>(WEAPON_REPLACEMENT_ERROR_CODES);

export function replacementErrorCode(error: unknown): string | null {
  return typeof error === "object" && error !== null && "code" in error && typeof error.code === "string"
    ? error.code
    : null;
}

export function isWeaponReplacementErrorCode(code: string): code is WeaponReplacementErrorCode {
  return weaponErrorCodeSet.has(code);
}

/**
 * 把后端稳定码翻译成“发生了什么 + 你能做什么”。
 * 武器二进制链路的码额外附带可复制的诊断码，方便用户反馈；通用流程错误不附带。
 */
export function replacementErrorMessage(
  error: unknown,
  fallback: string,
  errors: ReplacementCopy["errors"],
) {
  const code = replacementErrorCode(error);
  if (code === null) {
    return fallback;
  }

  if (isWeaponReplacementErrorCode(code)) {
    const copy = errors.weapon[code];
    return `${copy.message}${copy.hint ?? ""}${errors.diagnostic(code)}`;
  }

  const copy = errors.generic[code];
  if (copy === undefined) {
    return fallback;
  }
  return `${copy.message}${copy.hint ?? ""}`;
}
