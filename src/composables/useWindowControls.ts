import { onMounted } from "vue";

import {
  closeWindow,
  hydrateMaximized,
  minimizeWindow,
  toggleMaximize,
  windowMaximized,
} from "@/lib/windowChrome";

export function useWindowControls() {
  onMounted(() => {
    void hydrateMaximized();
  });

  return {
    isMaximized: windowMaximized,
    minimizeWindow,
    toggleMaximize,
    closeWindow,
  };
}
