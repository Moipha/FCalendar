import { defineStore } from "pinia";
import { computed, ref } from "vue";

import {
  bootstrapSession,
  getSession,
  setCurrentCalendar,
  syncCurrentCalendar,
  type ConnectionStatus,
  type SessionSnapshot,
} from "@/api/session";

export const useSessionStore = defineStore("session", () => {
  const snapshot = ref<SessionSnapshot | null>(null);
  const authToast = ref("");

  const currentCalendarId = computed(() => snapshot.value?.currentCalendarId ?? "");
  const connectionStatus = computed<ConnectionStatus>(
    () => snapshot.value?.connectionStatus ?? "logged_out",
  );
  const loggedIn = computed(() => Boolean(snapshot.value?.account));
  const pendingRemoteChanges = computed(() => snapshot.value?.pendingRemoteChanges ?? false);
  const calendars = computed(() => snapshot.value?.calendars ?? []);

  function apply(next: SessionSnapshot, toastAuth = false) {
    const prev = snapshot.value?.connectionStatus;
    snapshot.value = next;
    if (toastAuth && next.connectionStatus === "auth_error" && prev !== "auth_error") {
      authToast.value = "密码无效，请到设置重新输入";
    }
  }

  async function bootstrap() {
    const next = await bootstrapSession();
    apply(next, true);
    return next;
  }

  async function refresh() {
    apply(await getSession());
  }

  async function selectCalendar(id: string) {
    apply(await setCurrentCalendar(id));
    const remote = snapshot.value?.calendars.find((c) => c.id === id)?.accountId;
    if (remote && snapshot.value?.connectionStatus === "online") {
      try {
        apply(await syncCurrentCalendar());
      } catch {
        apply(await getSession(), true);
      }
    }
  }

  async function syncNow() {
    try {
      apply(await syncCurrentCalendar(), true);
    } catch (e) {
      apply(await getSession(), true);
      throw e;
    }
  }

  function clearAuthToast() {
    authToast.value = "";
  }

  return {
    snapshot,
    authToast,
    currentCalendarId,
    connectionStatus,
    loggedIn,
    pendingRemoteChanges,
    calendars,
    apply,
    bootstrap,
    refresh,
    selectCalendar,
    syncNow,
    clearAuthToast,
  };
});
