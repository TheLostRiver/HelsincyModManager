/*
 * 包内容查询的失败分档（`#354` 切片 D4-3）。
 *
 * 后端把稳定错误码原样透给前端（`package_contents_commands.rs` 的
 * `package_contents_error_to_command_error` 直接取 `PackageContentsQueryError::code()`，
 * 扫描类失败再透出 `ModPackageInstallFileScanError::code()`）。
 *
 * 这里只区分**有没有恢复路径**，不为每个码写一句话——玩家要的是「我现在能做什么」，
 * 不是一串码名。
 */

/** 记录在案的内容根已经不是这个包的合法候选。 */
const STALE_CONTENT_ROOT_CODE = "imported_mod_file_scan_stale_content_root_choice";

export type PackageContentsFailure =
  /**
   * 陈旧的内容根选择。
   *
   * 后端在这一档是**失败关闭**的：退回自动解析等于「玩家选了 A，我们装到 B」，装完不报错、
   * 文件落在别处，属于最难发现的一类。代价是整个查询失败——所以 UI 必须给出**出路**
   * （清除选择、重新解析），否则玩家看到的是个死胡同：包打不开，也不知道为什么。
   */
  | "stale-content-root"
  /** 其余失败（沙箱不可读、Mod 不存在、深度超限……）：只能重试或重新导入。 */
  | "generic";

/** 从 `invoke` 拒绝的原因里取稳定错误码；取不到返回 `null`。 */
export function packageContentsErrorCode(error: unknown): string | null {
  return typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof (error as { code: unknown }).code === "string"
    ? (error as { code: string }).code
    : null;
}

export function classifyPackageContentsError(error: unknown): PackageContentsFailure {
  return packageContentsErrorCode(error) === STALE_CONTENT_ROOT_CODE
    ? "stale-content-root"
    : "generic";
}
