/** Keep a measured menu inside the viewport, including a menu taller than the window. */
export function contextMenuPosition(
  anchor: { x: number; y: number },
  menu: { width: number; height: number },
  viewport: { width: number; height: number },
) {
  const padding = 8;
  return {
    left: Math.max(padding, Math.min(anchor.x, viewport.width - menu.width - padding)),
    top: Math.max(padding, Math.min(anchor.y, viewport.height - menu.height - padding)),
  };
}
