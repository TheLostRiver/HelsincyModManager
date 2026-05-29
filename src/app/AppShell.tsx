import {
  Archive,
  Crosshair,
  FileSearch,
  Gamepad2,
  LayoutDashboard,
  ListChecks,
  Moon,
  Puzzle,
  Settings,
  Sun,
  Tags,
  User,
} from "lucide-react";
import type { ComponentType, ReactNode } from "react";

type AppShellProps = {
  children: ReactNode;
};

type NavItem = {
  label: string;
  icon: ComponentType<{ size?: number; strokeWidth?: number }>;
  state?: "active" | "disabled";
};

const navItems: NavItem[] = [
  { label: "工作台", icon: LayoutDashboard, state: "active" },
  { label: "Mod 管理", icon: Puzzle, state: "disabled" },
  { label: "分类 / 标签", icon: Tags, state: "disabled" },
  { label: "Profile", icon: User, state: "disabled" },
  { label: "替换目标", icon: Crosshair, state: "disabled" },
  { label: "存档备份", icon: Archive, state: "disabled" },
  { label: "游戏管理", icon: Gamepad2 },
  { label: "任务队列", icon: ListChecks },
  { label: "日志 / 诊断", icon: FileSearch },
  { label: "设置", icon: Settings },
];

export function AppShell({ children }: AppShellProps) {
  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="主导航">
        <div className="brand-block">
          <h1>Helsincy</h1>
          <p>Mod Manager</p>
        </div>

        <nav className="nav-list">
          {navItems.map((item) => (
            <NavButton key={item.label} item={item} />
          ))}
        </nav>

        <div className="nav-footnote">
          <span aria-hidden="true" />
          <p>MHW:I&nbsp;&nbsp;首次启动</p>
        </div>
      </aside>

      <div className="app-surface">
        <TopStatusBar />
        <main className="workbench-body">{children}</main>
      </div>
    </div>
  );
}

function NavButton({ item }: { item: NavItem }) {
  const Icon = item.icon;
  const isActive = item.state === "active";

  return (
    <button
      type="button"
      className={`nav-item ${isActive ? "is-active" : ""}`}
      disabled={item.state === "disabled"}
    >
      {isActive && <span className="active-mark" aria-hidden="true" />}
      <Icon size={16} strokeWidth={2.1} />
      <span>{item.label}</span>
    </button>
  );
}

function TopStatusBar() {
  return (
    <header className="top-status-bar">
      <div className="current-game">
        <span>当前游戏</span>
        <strong>Monster Hunter: World - Iceborne</strong>
      </div>

      <div className="status-actions" aria-label="当前状态">
        <span className="status-pill warning">
          <span>Profile</span>
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
