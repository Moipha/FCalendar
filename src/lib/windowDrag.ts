import { getCurrentWindow } from "@tauri-apps/api/window";

import { toggleMaximize } from "@/lib/windowChrome";

const DRAG_THRESHOLD = 4;

function isInteractive(target: EventTarget | null) {
  return Boolean((target as HTMLElement | null)?.closest("button, a, input, textarea, [data-no-drag]"));
}

/** 双击可拖动区：最大化 / 还原。只在 dblclick 里切换，避免和第二下 pointerdown 重复触发。 */
export function onChromeDblClick(event: MouseEvent) {
  if (isInteractive(event.target)) {
    return;
  }
  event.preventDefault();
  event.stopPropagation();
  void toggleMaximize();
}

/**
 * 按下后移动超过阈值才 startDragging，避免：
 * 1. 单击被当成拖动
 * 2. 双击第二下 startDragging 把刚最大化的窗口立刻拖回原尺寸
 */
export function onChromeDragPointerDown(event: PointerEvent) {
  if (event.button !== 0 || isInteractive(event.target)) {
    return;
  }
  if (event.detail >= 2) {
    event.preventDefault();
    event.stopPropagation();
    return;
  }

  const startX = event.clientX;
  const startY = event.clientY;
  const onMove = (next: PointerEvent) => {
    if (Math.hypot(next.clientX - startX, next.clientY - startY) < DRAG_THRESHOLD) {
      return;
    }
    cleanup();
    void getCurrentWindow()
      .startDragging()
      .catch(() => {
        // 浏览器预览没有 Tauri 窗口
      });
  };
  const cleanup = () => {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", cleanup);
    window.removeEventListener("pointercancel", cleanup);
  };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", cleanup);
  window.addEventListener("pointercancel", cleanup);
}
