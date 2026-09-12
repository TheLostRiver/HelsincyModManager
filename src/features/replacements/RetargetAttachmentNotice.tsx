import { resolveCopy, useI18n } from "../../shared/i18n";
import type { ReinstallAttachmentCounts } from "../mods/modReinstallTypes";

const attachmentCopy = {
  zh_cn: {
    retained: (count: number) => `本次保留 ${count} 个已安装附件，路径和内容保持不变。`,
    excluded: (count: number) => `本次未包含 ${count} 个随包插件或工具，相关功能可能不可用。`,
  },
  en: {
    retained: (count: number) => `${count} installed companion ${count === 1 ? "file will keep its existing path and contents" : "files will keep their existing paths and contents"}.`,
    excluded: (count: number) => `${count} bundled plugin or tool ${count === 1 ? "file is" : "files are"} not included. Related features may be unavailable.`,
  },
  ja: {
    retained: (count: number) => `インストール済みの付属ファイル ${count} 個は、現在のパスと内容のまま保持されます。`,
    excluded: (count: number) => `同梱のプラグインまたはツール ${count} 個は含まれません。関連機能が利用できない場合があります。`,
  },
};

export function RetargetAttachmentNotice({ counts }: { counts?: ReinstallAttachmentCounts }) {
  const { locale } = useI18n();
  const copy = resolveCopy(attachmentCopy, locale);
  if (!counts) return null;
  return <>
    {counts.retained > 0 && <p className="replacement-panel__notice" role="status">{copy.retained(counts.retained)}</p>}
    {counts.excluded > 0 && <p className="replacement-panel__notice" role="status">{copy.excluded(counts.excluded)}</p>}
  </>;
}
