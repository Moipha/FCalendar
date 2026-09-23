/** 点遮罩 / 顶栏 / 左边栏都不关弹窗；点 Toast 仍可交互。 */
export function preventOverlayDismiss(event: Event) {
  const target = event.target as HTMLElement | null;
  if (target?.closest("[data-sonner-toaster]")) {
    return;
  }
  event.preventDefault();
}
