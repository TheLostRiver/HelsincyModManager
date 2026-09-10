import type { LocaleDictionary } from "../../shared/i18n";

type ModHoverCopy = {
  loading: string;
  unavailable: string;
  notProvided: string;
  author: string;
  version: string;
  notes: string;
  categories: string;
  tags: string;
  origin: string;
  nexusId: string;
};

export const modHoverCopy = {
  zh_cn: { loading: "正在读取 Mod 信息", unavailable: "Mod 信息暂不可用", notProvided: "未提供", author: "作者", version: "版本", notes: "备注", categories: "分类", tags: "标签", origin: "导入来源", nexusId: "NexusMods ID" },
  en: { loading: "Loading mod information", unavailable: "Mod information unavailable", notProvided: "Not provided", author: "Author", version: "Version", notes: "Notes", categories: "Categories", tags: "Tags", origin: "Import source", nexusId: "NexusMods ID" },
  ja: { loading: "Mod 情報を読み込み中", unavailable: "Mod 情報を取得できません", notProvided: "未登録", author: "作者", version: "バージョン", notes: "メモ", categories: "カテゴリ", tags: "タグ", origin: "インポート元", nexusId: "NexusMods ID" },
} satisfies LocaleDictionary<ModHoverCopy>;
