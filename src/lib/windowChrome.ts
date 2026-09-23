import { ref } from "vue";
import {
  PhysicalPosition,
  PhysicalSize,
  currentMonitor,
  getCurrentWindow,
} from "@tauri-apps/api/window";

type RestoreBounds = {
  width: number;
  height: number;
  x: number;
  y: number;
};

export const windowMaximized = ref(false);
let restoreBounds: RestoreBounds | null = null;

async function snapshotBounds() {
  const win = getCurrentWindow();
  const size = await win.innerSize();
  const position = await win.outerPosition();
  restoreBounds = {
    width: size.width,
    height: size.height,
    x: position.x,
    y: position.y,
  };
}

async function applyRestoreBounds() {
  if (!restoreBounds) {
    return;
  }
  const win = getCurrentWindow();
  await win.setSize(new PhysicalSize(restoreBounds.width, restoreBounds.height));
  await win.setPosition(new PhysicalPosition(restoreBounds.x, restoreBounds.y));
}

async function fillWorkArea() {
  const monitor = await currentMonitor();
  if (!monitor) {
    return;
  }
  const win = getCurrentWindow();
  await win.setPosition(monitor.workArea.position);
  await win.setSize(monitor.workArea.size);
}

export async function minimizeWindow() {
  await getCurrentWindow().minimize();
}

export async function closeWindow() {
  await getCurrentWindow().close();
}

export async function toggleMaximize() {
  const win = getCurrentWindow();
  if (windowMaximized.value) {
    try {
      await win.unmaximize();
    } catch {
      // 无边框 Windows 窗口常无法通过系统状态正常 unmaximize
    }
    await applyRestoreBounds();
    windowMaximized.value = false;
    return;
  }

  await snapshotBounds();
  try {
    await win.maximize();
  } catch {
    await fillWorkArea();
  }
  const nativeMaximized = await win.isMaximized().catch(() => false);
  if (!nativeMaximized) {
    await fillWorkArea();
  }
  windowMaximized.value = true;
}

export async function hydrateMaximized() {
  windowMaximized.value = await getCurrentWindow().isMaximized().catch(() => false);
}
