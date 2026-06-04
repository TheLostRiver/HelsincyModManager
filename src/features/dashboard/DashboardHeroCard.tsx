import { FolderOpen, Search } from "lucide-react";
import { supportCards } from "./dashboardData";

export function DashboardHeroCard() {
  return (
    <section className="setup-panel" aria-labelledby="setup-title">
      <div className="setup-message">
        <span className="badge warning">
          <span className="dot warning-dot" aria-hidden="true" />
          目录未配置
        </span>
        <h3 id="setup-title">未找到游戏目录</h3>
        <p>需要先识别《怪物猎人：世界 冰原》的安装目录，才能导入和安装 Mod。</p>
      </div>

      <div className="setup-actions">
        <button type="button" className="primary-action">
          <Search size={16} />
          自动扫描 Steam
        </button>
        <button type="button" className="secondary-action">
          <FolderOpen size={16} />
          手动选择游戏目录
        </button>
      </div>

      <div className="support-grid" aria-label="支持信息">
        {supportCards.map((card) => (
          <article className="support-card group" key={card.label}>
            <div className="support-card-header">
              <card.icon size={16} color={card.iconColor} strokeWidth={2.1} />
              <span>{card.label}</span>
            </div>
            <strong>{card.value}</strong>
          </article>
        ))}
      </div>
    </section>
  );
}
