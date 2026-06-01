import { Settings } from "lucide-react";
import { ThemeMenu } from "./ThemeMenu";

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
        <ThemeMenu />
        <button type="button" className="icon-button" aria-label="打开设置">
          <Settings size={16} />
        </button>
      </div>
    </header>
  );
}
