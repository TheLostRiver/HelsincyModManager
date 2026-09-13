import { resolveCopy, useI18n } from "../../shared/i18n";
import { pluginErrorMessage, pluginSelectionCopy } from "./pluginSelectionCopy";
import type { PluginSelectionController } from "./usePluginSelection";
import "./PluginSelectionPanel.css";

export type PluginSelectionView = Pick<PluginSelectionController, "status" | "inventory" | "error" | "saving" | "draft" | "reload" | "choose">;

export function PluginSelectionPanel({ controller, disabled = false }: { controller: PluginSelectionView; disabled?: boolean }) {
  const { locale } = useI18n();
  const copy = resolveCopy(pluginSelectionCopy, locale);
  if (controller.status === "loading") return <p role="status" className="plugin-selection__hint">{copy.loading}</p>;
  if (controller.status === "ready" && !controller.inventory && !controller.error) return null;
  return <section className="plugin-selection" aria-label={copy.title}>
    <h4>{copy.title}</h4>
    <p className="plugin-selection__hint">{controller.draft ? copy.draftHint : copy.hint}</p>
    {controller.error !== null && <div role="alert" className="plugin-selection__error">{pluginErrorMessage(controller.error, copy)} <button type="button" onClick={controller.reload} disabled={controller.saving}>{copy.retry}</button></div>}
    {controller.inventory && <>
      <ul>{controller.inventory.files.map((file) => <li key={file.fileId}>
        <label><input type="checkbox" checked={file.selected} disabled={disabled || controller.saving || !file.selectable}
          onChange={(event) => { void controller.choose(file.fileId, event.target.checked).catch(() => {}); }} />
          <span><code>{file.relativePath}</code><small>{file.excludedByPackage ? copy.packageExcluded : file.retainOnly ? copy.retained : copy.checks[file.check]}{file.managed ? ` · ${copy.managed}` : ""}</small></span>
        </label>
      </li>)}</ul>
      <p className="plugin-selection__hint">{copy.dependency}</p>
    </>}
    {controller.saving && <p role="status">{copy.saving}</p>}
  </section>;
}
