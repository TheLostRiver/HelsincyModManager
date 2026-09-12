import type { LocaleDictionary } from "../../shared/i18n";
import type { RetargetFileDisposition, RetargetFilePreview, RetargetFileReason } from "./retargetFileTypes";

type RetargetFileCopy = {
  previewReapply: string; reapplyTitle: string; confirmReapply: string; reapplyHint: string; noChanges: string;
  title: (count: number) => string; search: string; empty: string;
  source: string; installed: string; target: string; notManaged: string; excluded: string;
  equipment: string; package: string; unknown: string; more: string;
  shown: (visible: number, total: number) => string;
  dispositions: Record<RetargetFileDisposition, string>;
  reasons: Record<RetargetFileReason, string>;
  changes: Record<NonNullable<RetargetFilePreview["change"]>, string>;
};

export const retargetFileCopy = {
  zh_cn: {
    previewReapply: "重新应用当前目标", reapplyTitle: "重新应用预览", confirmReapply: "应用更新",
    reapplyHint: "保留当前装备目标，检查已安装文件是否需要按当前规则更新。",
    noChanges: "文件已符合当前规则，无需更新。",
    title: (count: number) => `文件去向（${count}）`, search: "搜索文件或装备", empty: "没有匹配的文件。",
    source: "原包路径", installed: "当前管理位置", target: "本次目标", notManaged: "此 Mod 无管理记录", excluded: "本次不包含",
    equipment: "装备资源", package: "包级资源", unknown: "未知处置", more: "显示更多文件",
    shown: (visible: number, total: number) => `显示 ${visible} / ${total} 个文件`,
    dispositions: { relocated: "映射到目标", kept_in_place: "保留作者原位", package_companion: "包级配套", installed_attachment_retained: "保留已安装附件", plugin_candidate: "未包含的插件候选", policy_excluded: "按策略排除" },
    reasons: {
      target_mapping: "按装备目标映射资源路径。", original_target: "使用作者设置的原始位置。",
      texture_reference: "保持贴图路径，保留既有引用。", unmapped_resource: "无法明确映射，保留原始路径和内容。",
      ambiguous_resource_identity: "名称包含多处装备编号，无法确定改名方式，保留完整原路径和内容。",
      conflicting_resource_identity: "目录或文件名中的装备编号与来源不一致，保留完整原路径和内容。",
      package_resource: "作为整个 Mod 的配套资源保留。", installed_attachment: "已核实的已安装附件，保持原路径和内容。",
      plugin_not_included: "此插件候选未包含在本次计划中，当前不会新增安装。", executable_policy: "随包可执行文件或脚本未包含在本次计划中。",
    },
    changes: { retained: "无需写入", replaced: "更新", added: "新增", stale: "清理旧文件" },
  },
  en: {
    previewReapply: "Reapply current targets", reapplyTitle: "Reapply preview", confirmReapply: "Apply updates",
    reapplyHint: "Keep the installed equipment targets and check whether files need updating under the current rules.",
    noChanges: "Files already match the current rules. No update is needed.",
    title: (count: number) => `File destinations (${count})`, search: "Search files or equipment", empty: "No matching files.",
    source: "Package path", installed: "Currently managed path", target: "Planned destination", notManaged: "Not managed by this Mod", excluded: "Not included",
    equipment: "Equipment resource", package: "Package resource", unknown: "Unknown disposition", more: "Show more files",
    shown: (visible: number, total: number) => `Showing ${visible} of ${total} files`,
    dispositions: { relocated: "Mapped to target", kept_in_place: "Original location", package_companion: "Package companion", installed_attachment_retained: "Installed companion retained", plugin_candidate: "Plugin candidate not included", policy_excluded: "Excluded by policy" },
    reasons: {
      target_mapping: "Resource paths follow the selected equipment target.", original_target: "Use the location set by the Mod author.",
      texture_reference: "Keep texture paths to preserve existing references.", unmapped_resource: "No unambiguous mapping is available; keep the original path and contents.",
      ambiguous_resource_identity: "The name contains multiple equipment IDs. Keep the entire original path and contents because the intended rename is unclear.",
      conflicting_resource_identity: "An equipment ID in the directory or filename disagrees with the source. Keep the entire original path and contents.",
      package_resource: "Keep this companion resource for the whole Mod.", installed_attachment: "The verified installed companion keeps its path and contents.",
      plugin_not_included: "This plugin candidate is outside the current plan and will not be newly installed.", executable_policy: "The bundled executable or script is outside the current plan.",
    },
    changes: { retained: "No write needed", replaced: "Update", added: "Add", stale: "Remove old file" },
  },
  ja: {
    previewReapply: "現在の対象に再適用", reapplyTitle: "再適用のプレビュー", confirmReapply: "更新を適用",
    reapplyHint: "現在の装備対象を維持し、最新のルールに合わせたファイル更新が必要か確認します。",
    noChanges: "ファイルは現在のルールに一致しています。更新は不要です。",
    title: (count: number) => `ファイルの配置先（${count}）`, search: "ファイルや装備を検索", empty: "一致するファイルはありません。",
    source: "パッケージ内のパス", installed: "現在の管理パス", target: "今回の配置先", notManaged: "この Mod の管理記録なし", excluded: "今回は含めません",
    equipment: "装備リソース", package: "パッケージリソース", unknown: "不明な処理", more: "さらに表示",
    shown: (visible: number, total: number) => `${total} 個中 ${visible} 個を表示`,
    dispositions: { relocated: "対象に配置", kept_in_place: "作者の元の位置を維持", package_companion: "パッケージの付属リソース", installed_attachment_retained: "インストール済みの付属ファイルを保持", plugin_candidate: "含まれないプラグイン候補", policy_excluded: "ポリシーによる除外" },
    reasons: {
      target_mapping: "選択した装備対象に合わせてパスを設定します。", original_target: "作者が指定した元の位置を使用します。",
      texture_reference: "既存の参照を維持するためテクスチャのパスを保持します。", unmapped_resource: "明確に対応付けられないため、元のパスと内容を保持します。",
      ambiguous_resource_identity: "名前に装備番号が複数あり改名方法を確定できないため、元のパス全体と内容を保持します。",
      conflicting_resource_identity: "ディレクトリまたはファイル名の装備番号が元の装備と一致しないため、元のパス全体と内容を保持します。",
      package_resource: "Mod 全体の付属リソースとして保持します。", installed_attachment: "確認済みの付属ファイルのパスと内容を保持します。",
      plugin_not_included: "このプラグイン候補は今回の計画に含まれず、新規にインストールされません。", executable_policy: "同梱の実行ファイルまたはスクリプトは今回の計画に含まれません。",
    },
    changes: { retained: "書き込み不要", replaced: "更新", added: "追加", stale: "旧ファイルを削除" },
  },
} satisfies LocaleDictionary<RetargetFileCopy>;
