import type { LocaleDictionary } from "../../shared/i18n";
import type { PluginCandidate } from "./pluginSelectionTypes";

type PluginCopy = {
  title: string; hint: string; draftHint: string; dependency: string; loading: string; saving: string;
  managed: string; retained: string; packageExcluded: string; retry: string; failed: string; noProfile: string;
  confirm: string; cancel: string; preview: string; apply: string; noChanges: string;
  checks: Record<PluginCandidate["check"], string>; errors: Record<string, string>;
};

export const pluginSelectionCopy = {
  zh_cn: {
    title: "插件与随包附件", hint: "选择会保存到当前配置档；确认安装或重新应用后才修改游戏文件。",
    draftHint: "选择仅作用于当前配置档，保存后生效。", dependency: "未声明具体功能依赖；跳过插件可能影响 Mod 的部分功能。",
    loading: "正在检查插件…", saving: "正在保存选择…", managed: "已管理", retained: "仅保留已安装文件",
    packageExcluded: "已在包文件设置中排除", retry: "重新检查", failed: "暂时无法读取插件选择，请重新检查。", noProfile: "选择配置档后可管理插件。",
    confirm: "确认并安装", cancel: "取消", preview: "预览插件变更", apply: "应用当前配置", noChanges: "当前文件无需更新。",
    checks: { supported: "64 位 DLL 结构检查通过", invalid_format: "无法识别为完整 DLL", unsupported_architecture: "不支持此 DLL 架构", not_dynamic_library: "不是 DLL 文件", policy_excluded: "随包工具或脚本，不新增安装" },
    errors: { plugin_selection_unavailable: "插件选择暂时不可用，请重新检查。", plugin_inventory_changed: "包内容、版本或选择已变化，请重新检查后再确认。", plugin_selection_required: "请先确认本次插件选择。",
      plugin_selection_invalid: "此选择不能应用，请重新检查可用文件。", plugin_source_unavailable: "无法读取此版本的插件文件。", plugin_manifest_unverified: "当前安装记录无法验证，请先处理安装状态。" },
  },
  en: {
    title: "Plugins and bundled files", hint: "Choices are saved for this profile. Game files change only after you confirm installation or reapply.",
    draftHint: "Choices apply to this profile after saving.", dependency: "Specific dependencies are not declared; skipping plugins may affect some Mod features.",
    loading: "Checking plugins…", saving: "Saving choices…", managed: "Managed", retained: "Keep installed file only",
    packageExcluded: "Excluded in package file settings", retry: "Check again", failed: "Plugin choices are unavailable. Please check again.", noProfile: "Select a profile to manage plugins.",
    confirm: "Confirm and install", cancel: "Cancel", preview: "Preview plugin changes", apply: "Apply current configuration", noChanges: "No file updates are needed.",
    checks: { supported: "64-bit DLL structure verified", invalid_format: "Not recognized as a complete DLL", unsupported_architecture: "Unsupported DLL architecture", not_dynamic_library: "Not a DLL", policy_excluded: "Bundled tool or script; not newly installed" },
    errors: { plugin_selection_unavailable: "Plugin choices are unavailable. Check again.", plugin_inventory_changed: "Package contents, version or choices changed. Check again before confirming.", plugin_selection_required: "Confirm the plugin choices first.",
      plugin_selection_invalid: "These choices cannot be applied. Check the available files again.", plugin_source_unavailable: "Plugin files for this revision cannot be read.", plugin_manifest_unverified: "The installation record cannot be verified. Resolve its state first." },
  },
  ja: {
    title: "プラグインと付属ファイル", hint: "選択は現在のプロファイルに保存されます。インストールまたは再適用を確定するまでゲームファイルは変更されません。",
    draftHint: "保存後、現在のプロファイルに選択が適用されます。", dependency: "具体的な依存関係は宣言されていません。除外すると Mod の一部機能に影響する場合があります。",
    loading: "プラグインを確認中…", saving: "選択を保存中…", managed: "管理済み", retained: "インストール済みファイルのみ保持",
    packageExcluded: "パッケージのファイル設定で除外済み", retry: "再確認", failed: "選択を読み取れません。再確認してください。", noProfile: "プロファイルを選択するとプラグインを管理できます。",
    confirm: "確定してインストール", cancel: "キャンセル", preview: "プラグインの変更を確認", apply: "現在の設定を適用", noChanges: "ファイルの更新は不要です。",
    checks: { supported: "64 ビット DLL の構造を確認済み", invalid_format: "完全な DLL として認識できません", unsupported_architecture: "未対応の DLL アーキテクチャ", not_dynamic_library: "DLL ではありません", policy_excluded: "付属ツールまたはスクリプトは新規インストールしません" },
    errors: { plugin_selection_unavailable: "プラグインの選択を利用できません。再確認してください。", plugin_inventory_changed: "内容、バージョンまたは選択が変わりました。再確認してから確定してください。", plugin_selection_required: "先にプラグインの選択を確定してください。",
      plugin_selection_invalid: "この選択は適用できません。利用可能なファイルを再確認してください。", plugin_source_unavailable: "このバージョンのプラグインを読み取れません。", plugin_manifest_unverified: "インストール記録を確認できません。先に状態を解決してください。" },
  },
} satisfies LocaleDictionary<PluginCopy>;

export function pluginErrorMessage(error: unknown, copy: PluginCopy): string {
  const code = typeof error === "object" && error !== null && "code" in error && typeof error.code === "string" ? error.code : "";
  return copy.errors[code] ?? copy.failed;
}
