import { LoaderCircle, Paperclip } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { PluginSelectionPanel, type PluginSelectionView } from "../install-plugins/PluginSelectionPanel";
import { pluginErrorMessage, pluginSelectionCopy } from "../install-plugins/pluginSelectionCopy";
import { retargetDialogCopy } from "./retargetDialogCopy";
import { RetargetPopover } from "./RetargetPopover";

export function RetargetPluginSelection({ controller, disabled }: { controller: PluginSelectionView; disabled: boolean }) {
  const { locale } = useI18n();
  const copy = resolveCopy(pluginSelectionCopy, locale);
  const dialogCopy = resolveCopy(retargetDialogCopy, locale);
  if (controller.status === "loading") return <span className="retarget-plugins__status" role="status"><LoaderCircle size={14} className="replacement-panel__spinner" aria-hidden="true" />{copy.loading}</span>;
  if (!controller.inventory && !controller.error) return null;
  const files = controller.inventory?.files ?? [];
  const selected = files.filter((file) => file.selected).length;
  const feedback = <>
    {controller.error !== null && <div role="alert" className="retarget-plugins__error">{pluginErrorMessage(controller.error, copy)}
      <button type="button" onClick={controller.reload} disabled={controller.saving}>{copy.retry}</button>
    </div>}
    {controller.saving && <span className="retarget-plugins__status" role="status">{copy.saving}</span>}
  </>;
  return <div className="retarget-plugins">
    {controller.inventory ? <RetargetPopover title={copy.title} feedback={feedback} trigger={<><Paperclip size={14} aria-hidden="true" /><span>{dialogCopy.attachments(selected, files.length - selected)}</span></>}>
      <PluginSelectionPanel controller={controller} disabled={disabled} showFeedback={false} />
    </RetargetPopover> : feedback}
  </div>;
}
