import type { LocaleDictionary } from "../../shared/i18n";
import type { ModLibrarySort, ModLibrarySortDirection, ModLibrarySortField } from "./modLibrarySort";

type SortCopy = {
  label: string;
  fieldLabel: string;
  directionLabel: string;
  fields: Record<ModLibrarySortField, string>;
  directions: Record<ModLibrarySortDirection, string>;
  reset: string;
  sizeHint: string;
  unknownHint: string;
  unknownSize: string;
  sizeLabel: string;
  options: Record<ModLibrarySort, string>;
};

export const modLibrarySortCopy: LocaleDictionary<SortCopy> = {
  zh_cn: {
    label: "排序", fieldLabel: "排序依据", directionLabel: "排序方向",
    fields: { name: "名称", imported_at: "导入时间", size: "文件大小" }, directions: { asc: "升序", desc: "降序" },
    reset: "恢复默认",
    sizeHint: "大小取当前版本解包后的内容总量", unknownHint: "缺少时间或大小的旧记录排在末尾",
    unknownSize: "大小未知", sizeLabel: "Mod 文件大小",
    options: { imported_at_desc: "最新导入", imported_at_asc: "最早导入", name_asc: "名称 · 升序", name_desc: "名称 · 降序", size_desc: "大小 · 从大到小", size_asc: "大小 · 从小到大" },
  },
  en: {
    label: "Sort", fieldLabel: "Sort by", directionLabel: "Sort direction",
    fields: { name: "Name", imported_at: "Import time", size: "File size" }, directions: { asc: "Ascending", desc: "Descending" },
    reset: "Reset to default",
    sizeHint: "Size of the current version's unpacked contents", unknownHint: "Legacy entries with unknown values appear last",
    unknownSize: "Size unknown", sizeLabel: "Mod file size",
    options: { imported_at_desc: "Newest imported", imported_at_asc: "Oldest imported", name_asc: "Name · ascending", name_desc: "Name · descending", size_desc: "Size · largest first", size_asc: "Size · smallest first" },
  },
  ja: {
    label: "並び順", fieldLabel: "並び替えの基準", directionLabel: "並び替えの方向",
    fields: { name: "名前", imported_at: "追加日時", size: "ファイルサイズ" }, directions: { asc: "昇順", desc: "降順" },
    reset: "既定に戻す",
    sizeHint: "現在のバージョンを展開した内容の合計サイズ", unknownHint: "日時やサイズが不明な項目は末尾に表示",
    unknownSize: "サイズ不明", sizeLabel: "Mod ファイルサイズ",
    options: { imported_at_desc: "追加した日時が新しい順", imported_at_asc: "追加した日時が古い順", name_asc: "名前 · 昇順", name_desc: "名前 · 降順", size_desc: "サイズ · 大きい順", size_asc: "サイズ · 小さい順" },
  },
};
