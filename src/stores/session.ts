import { defineStore } from "pinia";
import { computed, ref } from "vue";

import {
  bootstrapSession,
  getSession,
  setCurrentCalendar,
  syncCurrentCalendar,
  type ConnectionStatus,
  type SessionSnapshot,
  type SyncOutcome,
} from "@/api/session";
import { reportInvokeError } from "@/lib/invokeError";

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
  const syncing = ref(false);
  const currentCalendarRemote = computed(() => {
    const id = currentCalendarId.value;
    return Boolean(calendars.value.find((item) => item.id === id)?.accountId);
  });

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
        apply((await syncCurrentCalendar()).snapshot);
      } catch (e) {
        reportInvokeError("切换日历同步", e);
        apply(await getSession(), true);
      }
    }
  }

  async function syncNow(): Promise<SyncOutcome> {
    try {
      const outcome = await syncCurrentCalendar();
      apply(outcome.snapshot, true);
      return outcome;
    } catch (e) {
      reportInvokeError("同步当前日历", e);
      apply(await getSession(), true);
      throw e;
    }
  }

  function noteLocalMutation() {
    if (snapshot.value && currentCalendarRemote.value) {
      snapshot.value = { ...snapshot.value, pendingRemoteChanges: true };
    }
  }

  async function syncFromUi() {
    if (syncing.value) {
      return { kind: "busy" as const };
    }
    if (!loggedIn.value) {
      return { kind: "logged_out" as const };
    }
    if (!currentCalendarRemote.value) {
      return { kind: "local" as const };
    }
    if (connectionStatus.value === "auth_error") {
      return { kind: "auth_error" as const };
    }
    syncing.value = true;
    try {
      const outcome = await syncNow();
      return { kind: "ok" as const, pushed: outcome.pushed, pulled: outcome.pulled };
    } finally {
      syncing.value = false;
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
    syncing,
    currentCalendarRemote,
    apply,
    bootstrap,
    refresh,
    selectCalendar,
    syncNow,
    syncFromUi,
    noteLocalMutation,
    clearAuthToast,
  };
});
