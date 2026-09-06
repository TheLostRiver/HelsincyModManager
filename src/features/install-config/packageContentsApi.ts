import { invoke } from "@tauri-apps/api/core";
import type {
  ClearModPackageContentRootInput,
  ClearModPackageFileSelectionInput,
  GetModPackageContentsInput,
  PackageContents,
  SetModPackageContentRootInput,
  SetModPackageFileSelectionInput,
} from "./packageContentsTypes";

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

/**
 * 记下玩家为这个包选定的内容根（`#354` 切片 D2）。
 *
 * 选择**按包持久化**：提交安装时会从沙箱重建计划，重装同理，选择若只活在预览里，重建那
 * 一刻就没了。提交的值必须取自 `candidates`，后端在设置这一步即校验白名单。
 *
 * 与勾选不同，这一条**没有草稿**：内容根一改，整棵树的 `targetPath` 与 `installable` 全部
 * 重算，只能由后端回读给出。所以是选中即提交，界面拿回读结果重绘。
 */
export function setModPackageContentRoot(
  input: SetModPackageContentRootInput,
): Promise<PackageContents> {
  return invoke<PackageContents>("set_mod_package_content_root", {
    request: {
      gameId: input.gameId,
      modId: input.modId,
      contentRoot: input.contentRoot,
    },
  });
}

/** 撤销选择，回到自动解析。合集包会重新变回「等玩家决定」。 */
export function clearModPackageContentRoot(
  input: ClearModPackageContentRootInput,
): Promise<PackageContents> {
  return invoke<PackageContents>("clear_mod_package_content_root", {
    request: {
      gameId: input.gameId,
      modId: input.modId,
    },
  });
}

/**
 * 记下玩家勾掉的文件（`#354` 切片 D3）。
 *
 * 提交的是**要排除的**清单，**不是要保留的**：空清单 = 整包都装 = 计划逐字不变。
 * 用「保留清单」的话，包重新解压出的新文件会**静默不装**，而少装一个文件装完不报错。
 *
 * 后端**回读**并返回设置生效之后的结果，前端不必自己推演新状态。
 */
export function setModPackageFileSelection(
  input: SetModPackageFileSelectionInput,
): Promise<PackageContents> {
  return invoke<PackageContents>("set_mod_package_file_selection", {
    request: {
      gameId: input.gameId,
      modId: input.modId,
      excludedFiles: input.excludedFiles,
    },
  });
}

/** 撤销勾选，回到整包都装。同样回读并返回生效之后的结果。 */
export function clearModPackageFileSelection(
  input: ClearModPackageFileSelectionInput,
): Promise<PackageContents> {
  return invoke<PackageContents>("clear_mod_package_file_selection", {
    request: {
      gameId: input.gameId,
      modId: input.modId,
    },
  });
}
