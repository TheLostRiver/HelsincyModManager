import { invoke } from "@tauri-apps/api/core";
import type { GetModPackageContentsInput, PackageContents } from "./packageContentsTypes";

/**
 * 只读的包内容查询（`#354` 切片 D1）。
 *
 * 不写盘、不建计划、不改任何既有行为。与 `preview_imported_mod_install_plan` 的关键差别
 * 是**覆盖面**：后者只列内容根之下的文件，且包内有多个 `nativePC` 时直接报
 * `ambiguous_content_root` 而一个文件都不给；这条命令照常列出整包，把候选带在
 * `candidates` 里交给玩家决定。
 */
export function getModPackageContents(
  input: GetModPackageContentsInput,
): Promise<PackageContents> {
  return invoke<PackageContents>("get_mod_package_contents", {
    request: {
      gameId: input.gameId,
      modId: input.modId,
    },
  });
}
