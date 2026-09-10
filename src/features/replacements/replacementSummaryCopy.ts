import type { LocaleDictionary } from "../../shared/i18n";

type ReplacementSummaryCopy = {
  unknownName: string;
  source: string;
  installed: string;
  none: string;
  noBinding: string;
  unavailable: string;
  profileRequired: string;
  kinds: Record<string, string>;
};

export const replacementSummaryCopy = {
  zh_cn: {
    unknownName: "名称未知", source: "原始替换目标", installed: "当前重定向目标",
    none: "未识别到装备替换目标", noBinding: "暂无已安装的重定向目标", unavailable: "重定向信息暂不可用",
    profileRequired: "未选择配置档", kinds: { armor: "防具", weapon: "武器" },
  },
  en: {
    unknownName: "Unknown name", source: "Original targets", installed: "Current retargets",
    none: "No equipment targets detected", noBinding: "No installed retargets", unavailable: "Replacement information unavailable",
    profileRequired: "No profile selected", kinds: { armor: "Armor", weapon: "Weapon" },
  },
  ja: {
    unknownName: "名称不明", source: "元の置換対象", installed: "現在の変更先",
    none: "装備の置換対象は検出されませんでした", noBinding: "インストール済みの変更先はありません", unavailable: "置換情報を取得できません",
    profileRequired: "プロファイル未選択", kinds: { armor: "防具", weapon: "武器" },
  },
} satisfies LocaleDictionary<ReplacementSummaryCopy>;
