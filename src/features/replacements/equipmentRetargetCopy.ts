import type { LocaleDictionary } from "../../shared/i18n";

type EquipmentRetargetCopy = {
  title: string;
  hint: string;
  keep: string;
  selection: string;
  current: string;
};

export const equipmentRetargetCopy = {
  zh_cn: {
    title: "包内装备替换目标",
    hint: "分别选择每件装备要替换的对象。未调整的装备和配套资源会一起保留。",
    keep: "保持作者设定的替换对象",
    selection: "安装到",
    current: "当前安装替换对象",
  },
  en: {
    title: "Equipment replacement targets",
    hint: "Choose a target for each item. Unchanged equipment and companion resources are kept together.",
    keep: "Keep the author's original target",
    selection: "Install to",
    current: "Currently installed target",
  },
  ja: {
    title: "パッケージ内装備の置換先",
    hint: "装備ごとに置換先を選択します。変更しない装備と付属リソースも一緒に保持されます。",
    keep: "作者が設定した置換先を維持",
    selection: "インストール先",
    current: "現在のインストール先",
  },
} satisfies LocaleDictionary<EquipmentRetargetCopy>;
