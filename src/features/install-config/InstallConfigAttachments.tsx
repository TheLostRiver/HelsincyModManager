import { Paperclip } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { PluginSelectionPanel, type PluginSelectionView } from "../install-plugins/PluginSelectionPanel";
import { pluginErrorMessage, pluginSelectionCopy } from "../install-plugins/pluginSelectionCopy";
import { RetargetPopover } from "../replacements/RetargetPopover";
import { installConfigLayoutCopy } from "./installConfigLayoutCopy";

export function InstallConfigAttachments({ controller, disabled }: { controller: PluginSelectionView; disabled: boolean }) {
  const { locale } = useI18n();
  const copy = resolveCopy(pluginSelectionCopy, locale);
  const layoutCopy = resolveCopy(installConfigLayoutCopy, locale);
  const files = controller.inventory?.files ?? [];
  const tools = files.filter((file) => file.check === "policy_excluded" && !file.selected && !file.retainOnly && !file.selectable).length;
  const feedback = <>
    {controller.status === "loading" && <span role="status">{copy.loading}</span>}
    {controller.error !== null && <p role="alert">{pluginErrorMessage(controller.error, copy)} <button type="button" onClick={controller.reload} disabled={disabled || controller.saving}>{copy.retry}</button></p>}
    {controller.saving && <span role="status">{copy.saving}</span>}
  </>;
  return <div className="install-config-attachments">
    {files.length > 0 ? <RetargetPopover title={copy.title} trigger={<><Paperclip size={14} aria-hidden="true" />{tools === files.length ? layoutCopy.tools(tools) : layoutCopy.attachments(files.length)}</>} feedback={feedback}>
      <PluginSelectionPanel controller={controller} disabled={disabled} showFeedback={false} />
    </RetargetPopover> : feedback}
  </div>;
}
