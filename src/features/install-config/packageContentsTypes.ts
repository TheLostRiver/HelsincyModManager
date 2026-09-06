/*
 * `get_mod_package_contents` 族的前端类型（`#354` 切片 D1–D3 的契约投影）。
 *
 * 契约正文在 `docs/FRONTEND_BACKEND_CONTRACT.md` 第 5 节（安装计划预览）。
 */

/** 内容根的三档解析结果。`fallback` 与 `ambiguous` 不可混同——前者是「根已确定」，后者是「等玩家挑」。 */
export type PackageContentRootKind = "single" | "fallback" | "ambiguous";

export type PackageContentRoot = {
  kind: PackageContentRootKind;
  /**
   * 沙箱根相对路径。
   *
   * `fallback` 时是空串（内容根就是沙箱根本身），`ambiguous` 时为 `null`。
   * 空串与 `null` 是两回事，别用 falsy 判断合并它们。
   */
  path: string | null;
  /**
   * **仅** `ambiguous` 时非空的歧义候选。
   *
   * 想拿「这个包允许选哪些内容根」请用顶层的 `PackageContents.candidates`——
   * 那份才是恒有的白名单，玩家选定之后也不会消失。
   */
  candidates: string[];
};

export type PackageContentEntry = {
  /** 沙箱根相对路径，`/` 分隔。既是文件的稳定标识，也是树骨架的依据。 */
  packageFileId: string;
  sizeBytes: number;
  /** 相对内容根的安装路径；不在内容根之下、或内容根未定时为 `null`。 */
  targetPath: string | null;
  /** 能否落进本 game adapter 声明的允许安装根。 */
  installable: boolean;
  /**
   * 命中本游戏的「绝不安装」清单。
   *
   * 这是**事实**不是结论：拒绝清单当前只在重定向链路上被强制执行，普通安装链路尚未套用。
   * 因此不得把它渲染成「不会被安装」。
   */
  rejectedByGame: boolean;
  /** 玩家把这个文件勾掉了。「本游戏允许不允许」与「玩家要不要」是两件事，不要合并。 */
  excludedByPlayer: boolean;
};

export type PackageContents = {
  contentRoot: PackageContentRoot;
  /**
   * 这个包**允许**被选作内容根的全部目录，与 `contentRoot` 当前是哪个无关。
   *
   * 同时是 `set_mod_package_content_root` 的白名单：提交的值必须取自这里。
   */
  candidates: string[];
  /** 玩家勾掉的 `packageFileId`。整包仍逐条列在 `entries` 里——勾掉不等于看不见。 */
  excludedFiles: string[];
  entries: PackageContentEntry[];
};

export type GetModPackageContentsInput = {
  gameId: string;
  modId: string;
};

export type SetModPackageContentRootInput = {
  gameId: string;
  modId: string;
  /**
   * 沙箱根相对路径，**必须取自 `PackageContents.candidates`**——后端在设置这一步就校验
   * 白名单，不接受任意路径。空串合法，表示内容根就是沙箱根本身。
   */
  contentRoot: string;
};

export type ClearModPackageContentRootInput = {
  gameId: string;
  modId: string;
};

export type SetModPackageFileSelectionInput = {
  gameId: string;
  modId: string;
  /** 要**排除**的 `packageFileId`。空数组 = 整包都装。 */
  excludedFiles: string[];
};

export type ClearModPackageFileSelectionInput = {
  gameId: string;
  modId: string;
};
