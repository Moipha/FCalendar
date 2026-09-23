<script setup lang="ts">
import { useQuery, useQueryClient } from "@tanstack/vue-query";
import { computed, ref, watch } from "vue";

import { connectAccount, getAccount, rediscoverCalendars } from "@/api/account";
import {
  createRemoteCalendar,
  logoutAccount,
  updateAccountPassword,
} from "@/api/session";
import { useSessionStore } from "@/stores/session";
import { DEFAULT_JIEQI_COLORS, THEME_PRESETS, type JieqiSeason } from "@/lib/settingsTheme";
import {
  UPCOMING_DAY_OPTIONS,
  useSettingsStore,
  type AppSettings,
  type UpcomingDays,
} from "@/stores/settings";

const settings = useSettingsStore();
const session = useSessionStore();
const queryClient = useQueryClient();

type Draft = AppSettings;

const accountForm = ref({
  serverUrl: "http://127.0.0.1:5232",
  username: "",
  password: "",
});
const newPassword = ref("");
const newCalName = ref("");
const newCalColor = ref("#2563eb");
const migrateLocal = ref(false);
const clearLocalAfterMigrate = ref(false);
const logoutConfirm = ref(false);
const accountBusy = ref(false);
const accountError = ref("");
const online = computed(() => session.connectionStatus === "online");

const { refetch: refetchAccount } = useQuery({
  queryKey: ["account"],
  queryFn: getAccount,
  enabled: () => settings.dialogOpen,
});

const loggedIn = computed(() => session.loggedIn);

watch(
  () => settings.dialogOpen,
  (open) => {
    if (open) {
      accountError.value = "";
      void refetchAccount();
    }
  },
);

async function onConnectAccount() {
  accountError.value = "";
  accountBusy.value = true;
  try {
    await connectAccount({
      serverUrl: accountForm.value.serverUrl.trim(),
      username: accountForm.value.username.trim(),
      password: accountForm.value.password,
    });
    accountForm.value.password = "";
    await refetchAccount();
    await session.refresh();
    await queryClient.invalidateQueries({ queryKey: ["calendars"] });
  } catch (e) {
    accountError.value = e instanceof Error ? e.message : String(e);
  } finally {
    accountBusy.value = false;
  }
}

async function onUpdatePassword() {
  accountError.value = "";
  accountBusy.value = true;
  try {
    session.apply(await updateAccountPassword(newPassword.value), true);
    newPassword.value = "";
  } catch (e) {
    accountError.value = e instanceof Error ? e.message : String(e);
  } finally {
    accountBusy.value = false;
  }
}

async function onCreateCalendar() {
  accountError.value = "";
  accountBusy.value = true;
  try {
    session.apply(
      await createRemoteCalendar({
        displayName: newCalName.value.trim() || "日历",
        color: newCalColor.value,
        migrateFromLocal: migrateLocal.value,
        clearLocal: clearLocalAfterMigrate.value,
      }),
    );
    newCalName.value = "";
    newCalColor.value = "#2563eb";
    migrateLocal.value = false;
    clearLocalAfterMigrate.value = false;
    await queryClient.invalidateQueries({ queryKey: ["events"] });
    await queryClient.invalidateQueries({ queryKey: ["tasks"] });
    await queryClient.invalidateQueries({ queryKey: ["dayColors"] });
  } catch (e) {
    accountError.value = e instanceof Error ? e.message : String(e);
  } finally {
    accountBusy.value = false;
  }
}

async function onSelectCalendar(id: string) {
  await session.selectCalendar(id);
}

async function onRediscover() {
  accountError.value = "";
  accountBusy.value = true;
  try {
    await rediscoverCalendars();
    await refetchAccount();
    await session.refresh();
    await queryClient.invalidateQueries({ queryKey: ["calendars"] });
  } catch (e) {
    accountError.value = e instanceof Error ? e.message : String(e);
  } finally {
    accountBusy.value = false;
  }
}

async function onDisconnectAccount() {
  accountError.value = "";
  if (session.pendingRemoteChanges && !logoutConfirm.value) {
    logoutConfirm.value = true;
    return;
  }
  accountBusy.value = true;
  try {
    session.apply(await logoutAccount(logoutConfirm.value && session.pendingRemoteChanges));
    logoutConfirm.value = false;
    await refetchAccount();
    await queryClient.invalidateQueries({ queryKey: ["calendars"] });
    await queryClient.invalidateQueries({ queryKey: ["events"] });
    await queryClient.invalidateQueries({ queryKey: ["tasks"] });
  } catch (e) {
    accountError.value = e instanceof Error ? e.message : String(e);
  } finally {
    accountBusy.value = false;
  }
}

async function onSyncThenLogout() {
  accountBusy.value = true;
  accountError.value = "";
  try {
    await session.syncNow();
    if (!session.pendingRemoteChanges) {
      session.apply(await logoutAccount(false));
      logoutConfirm.value = false;
    }
  } catch (e) {
    accountError.value = e instanceof Error ? e.message : String(e);
  } finally {
    accountBusy.value = false;
  }
}

function accountSummary() {
  const a = session.snapshot?.account;
  if (!a) {
    return "";
  }
  return `${a.serverUrl} · ${a.username}`;
}

function copyFromStore(): Draft {
  return {
    upcomingDays: settings.upcomingDays,
    primaryColor: settings.primaryColor,
    jieqiColors: { ...settings.jieqiColors },
    weekStart: settings.weekStart,
    showMinorFestivals: settings.showMinorFestivals,
    scrollSnapWeeks: settings.scrollSnapWeeks,
  };
}

const draft = ref<Draft>(copyFromStore());

watch(
  () => settings.dialogOpen,
  (open) => {
    if (open) {
      draft.value = copyFromStore();
    }
  },
);

function setPrimary(hex: string) {
  draft.value.primaryColor = hex;
}

function setJieqi(season: JieqiSeason, hex: string) {
  draft.value.jieqiColors = { ...draft.value.jieqiColors, [season]: hex };
}

function resetJieqiDraft() {
  draft.value.jieqiColors = { ...DEFAULT_JIEQI_COLORS };
}

function cancel() {
  settings.closeDialog();
}

function confirm() {
  settings.patch({ ...draft.value });
  settings.persist();
  settings.closeDialog();
}

const jieqiRows: { season: JieqiSeason; label: string }[] = [
  { season: "spring", label: "春" },
  { season: "summer", label: "夏" },
  { season: "autumn", label: "秋" },
  { season: "winter", label: "冬" },
];
</script>

<template>
  <div
    v-if="settings.dialogOpen"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
  >
    <div
      class="fc-settings-dialog max-h-[min(90vh,720px)] w-full max-w-lg overflow-y-auto rounded-xl border border-border bg-background p-6 shadow-xl"
    >
      <h2 class="mb-5 text-lg font-semibold">设置</h2>

      <div class="space-y-8">
        <section>
          <h3 class="mb-3 text-sm font-medium">账户</h3>
          <p class="mb-3 text-xs text-muted-foreground">
            当前日历本是唯一读写目标。离线可切换日历，不能新建远端集合。
          </p>

          <div class="mb-4 space-y-2 text-sm">
            <p class="text-muted-foreground">当前日历本</p>
            <label
              v-for="cal in session.calendars"
              :key="cal.id"
              class="flex items-center gap-2"
            >
              <input
                type="radio"
                class="accent-primary"
                :checked="session.currentCalendarId === cal.id"
                @change="onSelectCalendar(cal.id)"
              />
              <span
                class="size-3 shrink-0 rounded-full border border-border"
                :style="cal.color ? { backgroundColor: cal.color } : undefined"
              />
              <span>{{ cal.displayName }}{{ cal.accountId ? "" : "（本地）" }}</span>
            </label>
          </div>

          <div v-if="loggedIn" class="space-y-3 text-sm">
            <p class="text-muted-foreground">{{ accountSummary() }}</p>
            <div class="space-y-2 rounded-md border border-border p-3">
              <p class="text-muted-foreground">新增远端日历本</p>
              <div class="flex items-center gap-2">
                <input
                  v-model="newCalName"
                  type="text"
                  class="min-w-0 flex-1 rounded-md border border-border bg-background px-3 py-2 disabled:opacity-50"
                  placeholder="名称"
                  :disabled="!online || accountBusy"
                />
                <input
                  v-model="newCalColor"
                  type="color"
                  class="h-10 w-10 shrink-0 cursor-pointer rounded-md border border-border bg-background p-1 disabled:opacity-50"
                  title="日历颜色"
                  :disabled="!online || accountBusy"
                />
              </div>
              <label class="flex items-center gap-2">
                <input v-model="migrateLocal" type="checkbox" class="accent-primary" :disabled="!online" />
                从本地迁移
              </label>
              <label v-if="migrateLocal" class="flex items-center gap-2">
                <input v-model="clearLocalAfterMigrate" type="checkbox" class="accent-primary" />
                迁移后清空本地
              </label>
              <button
                type="button"
                class="rounded-md border border-border px-3 py-1.5 disabled:opacity-50"
                :disabled="!online || accountBusy"
                @click="onCreateCalendar"
              >
                新建
              </button>
            </div>
            <label class="block">
              <span class="mb-1 block text-muted-foreground">更新密码</span>
              <input
                v-model="newPassword"
                type="password"
                class="w-full rounded-md border border-border bg-background px-3 py-2"
              />
            </label>
            <button
              type="button"
              class="rounded-md border border-border px-3 py-1.5 disabled:opacity-50"
              :disabled="accountBusy || !newPassword"
              @click="onUpdatePassword"
            >
              保存密码并重连
            </button>
            <div v-if="logoutConfirm" class="space-y-2 rounded-md border border-red-300 p-3 text-sm">
              <p>有未同步的远端改动。先同步，或放弃这些改动后退出。</p>
              <div class="flex flex-wrap gap-2">
                <button type="button" class="rounded-md border border-border px-3 py-1.5" @click="onSyncThenLogout">
                  先同步再退出
                </button>
                <button type="button" class="rounded-md border border-border px-3 py-1.5" @click="onDisconnectAccount">
                  放弃并退出
                </button>
                <button type="button" class="rounded-md px-3 py-1.5" @click="logoutConfirm = false">取消</button>
              </div>
            </div>
            <div class="flex flex-wrap gap-2">
              <button
                type="button"
                class="rounded-md border border-border px-3 py-1.5 text-sm disabled:opacity-50"
                :disabled="accountBusy"
                @click="onRediscover"
              >
                重新发现
              </button>
              <button
                type="button"
                class="rounded-md border border-border px-3 py-1.5 text-sm disabled:opacity-50"
                :disabled="accountBusy"
                @click="onDisconnectAccount"
              >
                退出登录
              </button>
            </div>
          </div>

          <div v-else class="space-y-3 text-sm">
            <label class="block">
              <span class="mb-1 block text-muted-foreground">服务器</span>
              <input
                v-model="accountForm.serverUrl"
                type="url"
                class="w-full rounded-md border border-border bg-background px-3 py-2"
                placeholder="http://127.0.0.1:5232"
                autocomplete="off"
              />
            </label>
            <label class="block">
              <span class="mb-1 block text-muted-foreground">用户名</span>
              <input
                v-model="accountForm.username"
                type="text"
                class="w-full rounded-md border border-border bg-background px-3 py-2"
                autocomplete="username"
              />
            </label>
            <label class="block">
              <span class="mb-1 block text-muted-foreground">密码</span>
              <input
                v-model="accountForm.password"
                type="password"
                class="w-full rounded-md border border-border bg-background px-3 py-2"
                autocomplete="current-password"
              />
            </label>
            <button
              type="button"
              class="rounded-md bg-primary px-4 py-2 text-primary-foreground disabled:opacity-50"
              :disabled="accountBusy || !accountForm.username || !accountForm.password"
              @click="onConnectAccount"
            >
              连接
            </button>
          </div>

          <p v-if="accountError" class="mt-2 text-sm text-red-600">{{ accountError }}</p>
        </section>

        <section>
          <h3 class="mb-3 text-sm font-medium">外观</h3>
          <div class="space-y-4">
            <div>
              <p class="mb-2 text-sm text-muted-foreground">主题色</p>
              <div class="flex flex-wrap items-center gap-2">
                <input
                  v-model="draft.primaryColor"
                  type="color"
                  class="size-9 cursor-pointer rounded border border-border bg-transparent p-0.5"
                  aria-label="主题色"
                />
                <button
                  v-for="preset in THEME_PRESETS"
                  :key="preset.id"
                  type="button"
                  class="size-8 rounded-md border border-border ring-offset-background transition hover:scale-105 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  :style="{ backgroundColor: preset.hex }"
                  :title="preset.label"
                  @click="setPrimary(preset.hex)"
                />
              </div>
            </div>

            <div>
              <p class="mb-2 text-sm text-muted-foreground">节气字色</p>
              <div class="space-y-2">
                <div
                  v-for="row in jieqiRows"
                  :key="row.season"
                  class="flex items-center gap-3 text-sm"
                >
                  <span class="w-6 shrink-0 text-muted-foreground">{{ row.label }}</span>
                  <input
                    :value="draft.jieqiColors[row.season]"
                    type="color"
                    class="size-8 cursor-pointer rounded border border-border bg-transparent p-0.5"
                    @input="setJieqi(row.season, ($event.target as HTMLInputElement).value)"
                  />
                  <span class="font-mono text-xs text-muted-foreground">{{
                    draft.jieqiColors[row.season]
                  }}</span>
                </div>
              </div>
              <button
                type="button"
                class="mt-2 text-xs text-muted-foreground underline-offset-2 hover:underline"
                @click="resetJieqiDraft"
              >
                恢复默认节气色
              </button>
            </div>
          </div>
        </section>

        <section>
          <h3 class="mb-3 text-sm font-medium">日历</h3>
          <div class="space-y-4">
            <div>
              <p class="mb-2 text-sm text-muted-foreground">一周从</p>
              <label class="mr-4 inline-flex items-center gap-2 text-sm">
                <input v-model="draft.weekStart" type="radio" value="monday" class="accent-primary" />
                周一
              </label>
              <label class="inline-flex items-center gap-2 text-sm">
                <input v-model="draft.weekStart" type="radio" value="sunday" class="accent-primary" />
                周日
              </label>
            </div>

            <label class="flex items-center gap-2 text-sm">
              <input v-model="draft.showMinorFestivals" type="checkbox" class="accent-primary" />
              显示次要节日
            </label>

            <label class="flex items-center gap-2 text-sm">
              <input v-model="draft.scrollSnapWeeks" type="checkbox" class="accent-primary" />
              滚动贴合周边界
            </label>
          </div>
        </section>

        <section>
          <h3 class="mb-3 text-sm font-medium">概览</h3>
          <p class="mb-2 text-sm text-muted-foreground">「待进行」列表显示范围</p>
          <label
            v-for="option in UPCOMING_DAY_OPTIONS"
            :key="option"
            class="flex items-center gap-2 text-sm"
          >
            <input
              v-model="draft.upcomingDays"
              type="radio"
              class="accent-primary"
              :value="option as UpcomingDays"
            />
            {{ option }} 天
          </label>
        </section>
      </div>

      <div class="mt-8 flex justify-end gap-2 border-t border-border pt-4">
        <button type="button" class="rounded-md border border-border px-4 py-2" @click="cancel">
          取消
        </button>
        <button
          type="button"
          class="rounded-md bg-primary px-4 py-2 text-primary-foreground"
          @click="confirm"
        >
          确定
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.fc-settings-dialog :deep(input[type="checkbox"]),
.fc-settings-dialog :deep(input[type="radio"]) {
  accent-color: var(--primary);
}
</style>
