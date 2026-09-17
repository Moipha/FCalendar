import { onMounted, ref } from "vue";
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

export function useWindowControls() {
  const isMaximized = ref(false);
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

  async function minimizeWindow() {
    await getCurrentWindow().minimize();
  }

  async function toggleMaximize() {
    const win = getCurrentWindow();
    if (isMaximized.value) {
      try {
        await win.unmaximize();
      } catch {
        // undecorated Windows windows often cannot unmaximize via OS state
      }
      await applyRestoreBounds();
      isMaximized.value = false;
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
    isMaximized.value = true;
  }

  async function closeWindow() {
    await getCurrentWindow().close();
  }

  onMounted(async () => {
    isMaximized.value = await getCurrentWindow().isMaximized().catch(() => false);
  });

  return {
    isMaximized,
    minimizeWindow,
    toggleMaximize,
    closeWindow,
  };
}
