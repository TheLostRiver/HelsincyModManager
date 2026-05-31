import { LayoutDashboard } from "lucide-react";
import { previewCards } from "./dashboardData";

export function DashboardModulePreview() {
  return (
    <section className="preview-panel" aria-labelledby="preview-title">
      <h3 id="preview-title">完成设置后将显示</h3>
      <p>以下模块会在目录识别、权限校验和默认配置档案创建后启用。</p>

      <div className="preview-heading">
        <LayoutDashboard size={16} />
        <strong>设置完成后启用</strong>
      </div>

      <div className="preview-grid">
        {previewCards.map((card) => (
          <article className="preview-card" key={card.label}>
            <strong>{card.label}</strong>
            <span className="skeleton-line" />
            <span className="skeleton-line short" style={{ width: card.shortWidth }} />
          </article>
        ))}
      </div>
    </section>
  );
}
