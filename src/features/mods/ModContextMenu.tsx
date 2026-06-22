import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import "./ModContextMenu.css";

export type ModContextMenuProps = {
  x: number;
  y: number;
  modId: string;
  onClose: () => void;
  onAction: (actionId: string, modId: string) => void;
};

// SVG Icons directly embedded for performance and simplicity
const IconPower = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M18.36 6.64a9 9 0 1 1-12.73 0"></path>
    <line x1="12" y1="2" x2="12" y2="12"></line>
  </svg>
);

const IconSettings = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="12" r="3"></circle>
    <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
  </svg>
);

const IconEdit = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
    <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
  </svg>
);

const IconLink = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
    <polyline points="15 3 21 3 21 9"></polyline>
    <line x1="10" y1="14" x2="21" y2="3"></line>
  </svg>
);

const IconFolder = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
  </svg>
);

export function ModContextMenu({ x, y, modId, onClose, onAction }: ModContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);

  // Keep ref up to date so we don't need to re-bind event listeners
  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  // Close when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onCloseRef.current();
      }
    };

    // Close when pressing Escape
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onCloseRef.current();
      }
    };

    // Bind listeners synchronously since propagation is stopped at the source
    // Use mousedown instead of click to prevent race conditions with React state updates
    window.addEventListener("mousedown", handleClickOutside);
    window.addEventListener("contextmenu", handleClickOutside);
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("mousedown", handleClickOutside);
      window.removeEventListener("contextmenu", handleClickOutside);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []); // Empty dependency array: bind only once!

  // Adjust position to stay within viewport
  const getStyle = (): React.CSSProperties => {
    let finalX = x;
    let finalY = y;

    // We roughly estimate menu size if ref isn't attached yet,
    // but a better way is to measure it after mount. For simplicity
    // and given its fixed contents, we can assume ~200x200 max.
    const menuWidth = 220;
    const menuHeight = 220;

    if (typeof window !== "undefined") {
      if (finalX + menuWidth > window.innerWidth) {
        finalX = window.innerWidth - menuWidth - 8;
      }
      if (finalY + menuHeight > window.innerHeight) {
        finalY = window.innerHeight - menuHeight - 8;
      }
    }

    return {
      left: `${finalX}px`,
      top: `${finalY}px`,
    };
  };

  const handleItemClick = (actionId: string) => {
    onAction(actionId, modId);
    onClose();
  };

  return createPortal(
    <div className="mod-context-menu" style={getStyle()} ref={menuRef}>
      <div
        className="mod-context-menu__item is-danger"
        onClick={() => handleItemClick("toggle-enable")}
      >
        <IconPower /> 启用 / 禁用
      </div>
      <div className="mod-context-menu__divider" />
      <div
        className="mod-context-menu__item"
        onClick={() => handleItemClick("info-settings")}
      >
        <IconSettings /> MOD 信息设置
      </div>
      <div
        className="mod-context-menu__item"
        onClick={() => handleItemClick("edit-files")}
      >
        <IconEdit /> MOD 文件修改
      </div>
      <div className="mod-context-menu__divider" />
      <div
        className="mod-context-menu__item"
        onClick={() => handleItemClick("open-nexus")}
      >
        <IconLink /> 跳到 NexusMods
      </div>
      <div
        className="mod-context-menu__item"
        onClick={() => handleItemClick("open-folder")}
      >
        <IconFolder /> 打开 MOD 文件夹
      </div>
    </div>,
    document.body
  );
}
