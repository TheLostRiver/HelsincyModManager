import { resolveCopy, useI18n } from "../../shared/i18n";
import { aboutPageCopy } from "./aboutPageCopy";
import type { UpdateCheckView } from "./updateCheckView";

export function UpdateCheckStatus({ view }: { view: UpdateCheckView }) {
  const { locale } = useI18n();
  const copy = resolveCopy(aboutPageCopy, locale).release;
  const message = view.kind === "update_available"
    ? copy.updateAvailable(view.version)
    : {
      idle: copy.notChecked,
      checking: copy.checking,
      unavailable: copy.unavailable,
      no_release: copy.noRelease,
      up_to_date: copy.upToDate,
    }[view.kind];
  return (
    <p className={`about-page__update-status is-${view.kind}`} role="status" aria-live="polite" aria-atomic="true">
      {message}
    </p>
  );
}
