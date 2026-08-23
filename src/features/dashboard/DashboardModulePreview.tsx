import { LayoutDashboard } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { dashboardCopy } from "./dashboardCopy";
import { previewCards } from "./dashboardData";

export function DashboardModulePreview() {
  const { locale } = useI18n();
  const copy = resolveCopy(dashboardCopy, locale).modulePreview;

  return (
    <section className="preview-panel" aria-labelledby="preview-title">
      <h3 id="preview-title">{copy.title}</h3>
      <p>{copy.description}</p>

      <div className="preview-heading">
        <LayoutDashboard size={16} />
        <strong>{copy.heading}</strong>
      </div>

      <div className="preview-grid">
        {previewCards.map((card) => (
          <article className="preview-card" key={card.labelKey}>
            <strong>{copy.cards[card.labelKey]}</strong>
            <span className="skeleton-line" />
            <span className="skeleton-line short" style={{ width: card.shortWidth }} />
          </article>
        ))}
      </div>
    </section>
  );
}
