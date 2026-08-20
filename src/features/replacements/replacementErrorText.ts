/**
 * 替换目标（Armor / Weapon retarget）错误文案映射。
 *
 * 后端把稳定错误码原样透传给前端（src-tauri/src/replacement_commands.rs 的
 * analysis_error_to_command_error 对 AnalysisRejected { code } 直接返回 code），
 * 因此前端必须覆盖 hmm-games-mhw 侧 WeaponAnalysisError / WeaponBinaryError 的全部码，
 * 否则用户只会看到无信息量的兜底文案。
 *
 * 三层闸门共同防止新码悄悄退回兜底提示：
 * 1. 本文件的 Record<WeaponReplacementErrorCode, ErrorCopy> —— 码表里有码却没写文案时 tsc 失败。
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

type ErrorCopy = {
  /** 发生了什么。 */
  message: string;
  /** 用户可以采取的下一步动作。 */
  hint?: string;
};

const weaponErrorCodeSet = new Set<string>(WEAPON_REPLACEMENT_ERROR_CODES);

/**
 * 按“用户可采取的行动”分三组：
 * 1. Mod 的模型文件本身不受支持 -> 换目标没用，只能装回原武器或找作者。
 * 2. 压缩包结构不受支持 -> 重新下载或拆包后再导入。
 * 3. 所选目标不匹配 -> 换一个目标即可。
 */
const weaponErrorCopy: Record<WeaponReplacementErrorCode, ErrorCopy> = {
  // 1. 模型 / 材质文件本身不受支持
  weapon_binary_format_invalid: {
    message: "该武器 Mod 的模型文件格式无法识别，可能不是 Iceborne 版本或文件已损坏。",
    hint: "可以重新下载该 Mod，或直接安装到它原本的武器上。",
  },
  weapon_binary_pair_incompatible: {
    message: "该武器 Mod 的模型与材质文件不匹配，无法安全改到其他武器上。",
    hint: "可以重新下载该 Mod，或直接安装到它原本的武器上。",
  },
  weapon_binary_reference_unsafe: {
    message: "该武器 Mod 的材质引用包含不安全的路径，已阻止写入。",
    hint: "请向 Mod 作者反馈，或直接安装到它原本的武器上。",
  },
  weapon_binary_reference_ambiguous: {
    message: "该武器 Mod 的材质引用无法唯一对应到目标武器，已阻止写入。",
    hint: "请向 Mod 作者反馈，或直接安装到它原本的武器上。",
  },
  weapon_binary_path_too_long: {
    message: "改到该目标后材质路径会超出游戏格式上限，已阻止写入。",
    hint: "请改选其他目标武器再试。",
  },
  weapon_transformer_output_invalid: {
    message: "转换结果未通过完整性校验，已阻止写入游戏目录。",
    hint: "请重试；若持续出现，请连同下方诊断码反馈。",
  },

  // 2. 压缩包结构不受支持
  weapon_source_not_found: {
    message: "该 Mod 中没有找到可识别的武器资源。",
    hint: "请确认它是 nativePC/wp 结构的 MHW:I 武器 Mod。",
  },
  weapon_multiple_source_roots: {
    message: "该压缩包包含多套武器资源，当前版本无法自动重定向。",
    hint: "请拆分成单套武器后分别导入。",
  },
  weapon_mixed_family: {
    message: "该压缩包混合了多种武器类型，当前版本无法自动重定向。",
    hint: "请拆分成单一武器类型后分别导入。",
  },
  weapon_mixed_install_payload: {
    message: "该压缩包除武器资源外还包含其他会写入游戏的内容，已阻止自动重定向。",
    hint: "请只保留武器资源后重新导入。",
  },
  weapon_incomplete_binary_pair: {
    message: "该武器 Mod 缺少成对的模型或材质文件。",
    hint: "请重新下载完整的 Mod 包。",
  },
  weapon_unknown_part: {
    message: "该武器 Mod 包含当前版本无法识别的部件。",
    hint: "当前版本只支持武器主体与已知副件（盾、鞘、副手等）。",
  },
  weapon_unsupported_resource: {
    message: "该武器 Mod 包含当前版本不支持的资源类型。",
    hint: "当前版本只支持 .mod3 与 .mrl3 文件。",
  },
  weapon_unsafe_path: {
    message: "该 Mod 包内存在不安全的文件路径，已阻止处理。",
    hint: "请重新下载该 Mod。",
  },
  weapon_duplicate_asset_path: {
    message: "该 Mod 包内存在重复的文件路径。",
    hint: "请重新下载该 Mod。",
  },
  weapon_case_insensitive_path_collision: {
    message: "该 Mod 包内存在仅大小写不同的冲突文件路径。",
    hint: "请重新下载该 Mod。",
  },
  weapon_invalid_package_file_id: {
    message: "该 Mod 的导入记录已失效。",
    hint: "请重新导入该 Mod。",
  },
  weapon_duplicate_package_file_id: {
    message: "该 Mod 的导入记录存在重复条目。",
    hint: "请重新导入该 Mod。",
  },
  weapon_identity_invalid: {
    message: "无法确认该武器资源的身份，已阻止处理。",
    hint: "请重新导入该 Mod。",
  },

  // 3. 所选目标不匹配
  weapon_cross_family_target: {
    message: "所选目标与该 Mod 的武器类型不同，当前版本不支持跨武器类型改装。",
    hint: "请选择同一类武器作为目标。",
  },
  weapon_unknown_family: {
    message: "该武器类型不在当前支持范围内。",
    hint: "当前版本支持 MHW:I 的 14 类武器。",
  },
  weapon_invalid_main_id: {
    message: "该武器资源编号不符合游戏规范，已阻止处理。",
    hint: "请重新下载该 Mod。",
  },
};

/**
 * 通用替换流程错误，不属于武器二进制链路，因此不展示诊断码。
 *
 * 这些码是 replacement_commands.rs 里散落的字面量，没有单一枚举可以穷尽，
 * 所以拿不到武器码那样的编译期闸门。覆盖率由 replacementErrorCodeContract.test.mjs
 * 按命名约定扫描该文件来兜底。
 */
const genericErrorCopy: Record<string, ErrorCopy> = {
  replacement_mod_not_found: { message: "未找到已导入的 Mod。" },
  replacement_package_unavailable: { message: "导入包当前不可用。" },
  replacement_source_not_retargetable: {
    message: "该 Mod 不是当前可自动处理的单源外观包。",
  },
  replacement_target_catalog_unavailable: { message: "替换目标目录暂不可用。" },
  replacement_analysis_unavailable: { message: "替换分析暂不可用。" },
  weapon_developer_seed_unavailable: {
    message: "武器替换仅在受控开发 Sandbox 中可用。",
  },
  weapon_source_content_unavailable: {
    message: "无法读取受控武器资源。",
    hint: "请重新导入该 Mod。",
  },
  replacement_target_not_found: { message: "所选替换目标已不存在。" },
  replacement_install_state_unavailable: { message: "无法确认当前安装状态。" },
  replacement_initial_install_blocked: {
    message: "当前安装或恢复状态不允许首次替换安装。",
  },
  replacement_installed_binding_unavailable: {
    message: "无法确认当前已安装的替换目标。",
  },
  replacement_target_already_selected: { message: "当前目标已安装。" },
  replacement_preview_unavailable: { message: "替换预览暂不可用。" },
  replacement_reinstall_preview_unavailable: {
    message: "无法读取目标切换预览的完整信息。",
    hint: "请刷新后重试；若持续出现，请连同当前 Mod 与目标一起反馈。",
  },
  // 入参校验：正常操作不会触发，出现时多半是列表或选择状态已过期。
  replacement_mod_id_invalid: {
    message: "Mod 标识无效，无法定位该 Mod。",
    hint: "请刷新 Mod 列表后重试。",
  },
  replacement_profile_id_invalid: {
    message: "Profile 标识无效。",
    hint: "请重新选择 Profile 后重试。",
  },
  replacement_target_id_invalid: {
    message: "替换目标标识无效。",
    hint: "请重新选择目标后重试。",
  },
  replacement_layer_invalid: {
    message: "安装层参数无效，已阻止本次操作。",
    hint: "请刷新后重试；若持续出现，请反馈。",
  },
  plan_token_invalid: {
    message: "目标切换预览已失效。",
    hint: "请重新生成预览。",
  },
  task_cannot_be_cancelled: {
    message: "任务已进入提交阶段，无法取消。",
    hint: "请等待执行结果。",
  },
  task_not_found: { message: "无法确认当前任务。", hint: "请刷新状态。" },
  reinstall_catalog_unavailable: { message: "目标切换预览暂不可用。" },
  reinstall_manifest_unavailable: { message: "目标切换预览暂不可用。" },
  reinstall_recovery_unavailable: { message: "目标切换预览暂不可用。" },
  reinstall_candidate_plan_unavailable: { message: "目标切换预览暂不可用。" },
  replacement_unsupported_game: { message: "当前游戏不支持替换目标。" },
};

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
export function replacementErrorMessage(error: unknown, fallback: string) {
  const code = replacementErrorCode(error);
  if (code === null) {
    return fallback;
  }

  if (isWeaponReplacementErrorCode(code)) {
    const copy = weaponErrorCopy[code];
    return `${copy.message}${copy.hint ?? ""}（诊断码：${code}）`;
  }

  const copy = genericErrorCopy[code];
  if (copy === undefined) {
    return fallback;
  }
  return `${copy.message}${copy.hint ?? ""}`;
}
