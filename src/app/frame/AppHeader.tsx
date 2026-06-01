import { Moon, Settings, Sun } from "lucide-react";

export function AppHeader() {
  return (
    <header className="top-status-bar">
      <div className="current-game">
        <span>当前游戏</span>
        <strong>Monster Hunter: World - Iceborne</strong>
      </div>

      <div className="status-actions" aria-label="当前状态">
        <span className="status-pill warning">
          <span>配置档</span>
          <strong>待初始化</strong>
        </span>
        <span className="status-pill warning compact">
          <span className="dot warning-dot" aria-hidden="true" />
          <strong>目录未配置</strong>
        </span>
        <span className="status-pill neutral compact">
          <span className="dot neutral-dot" aria-hidden="true" />
          <span>任务空闲</span>
        </span>
      </div>

      <div className="window-tools" aria-label="窗口工具">
        <div className="theme-toggle" aria-label="主题模式">
          <button type="button" className="theme-button is-selected" aria-label="浅色主题">
            <Sun size={14} />
          </button>
          <button type="button" className="theme-button" aria-label="深色主题">
            <Moon size={14} />
          </button>
        </div>
        <button type="button" className="icon-button" aria-label="打开设置">
          <Settings size={16} />
        </button>
      </div>
    </header>
  );
}
