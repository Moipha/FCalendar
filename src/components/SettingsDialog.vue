<script setup lang="ts">
import { Loader2 } from "@lucide/vue";
import { useQuery, useQueryClient } from "@tanstack/vue-query";
import { computed, ref, watch } from "vue";

import { connectAccount, getAccount, rediscoverCalendars } from "@/api/account";
import {
  createRemoteCalendar,
  logoutAccount,
  updateAccountPassword,
} from "@/api/session";
import ColorSwatchInput from "@/components/ColorSwatchInput.vue";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { ScrollArea } from "@/components/ui/scroll-area";
import { preventOverlayDismiss } from "@/lib/dialogDismiss";
import { reportInvokeError } from "@/lib/invokeError";
import { DEFAULT_JIEQI_COLORS, THEME_PRESETS, type JieqiSeason } from "@/lib/settingsTheme";
import { useSessionStore } from "@/stores/session";
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
type SettingsPage = "account" | "appearance" | "calendar" | "overview";

const page = ref<SettingsPage>("account");
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
const logoutAlertOpen = ref(false);
const accountBusy = ref(false);
const accountError = ref("");
const online = computed(() => session.connectionStatus === "online");

const { refetch: refetchAccount } = useQuery({
  queryKey: ["account"],
  queryFn: getAccount,
  enabled: () => settings.dialogOpen,
});

const loggedIn = computed(() => session.loggedIn);

const navItems: { id: SettingsPage; label: string }[] = [
  { id: "account", label: "账户" },
  { id: "appearance", label: "外观" },
  { id: "calendar", label: "日历" },
  { id: "overview", label: "概览" },
];

watch(
  () => settings.dialogOpen,
  (open) => {
    if (open) {
      accountError.value = "";
      page.value = "account";
      logoutAlertOpen.value = false;
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
    accountError.value = reportInvokeError("连接账户", e);
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
    accountError.value = reportInvokeError("更新密码", e);
    await session.refresh();
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
    accountError.value = reportInvokeError("新建日历", e);
    await session.refresh();
    await queryClient.invalidateQueries({ queryKey: ["events"] });
    await queryClient.invalidateQueries({ queryKey: ["tasks"] });
    await queryClient.invalidateQueries({ queryKey: ["dayColors"] });
    await queryClient.invalidateQueries({ queryKey: ["calendars"] });
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
    accountError.value = reportInvokeError("重新发现", e);
  } finally {
    accountBusy.value = false;
  }
}

async function requestLogout() {
  accountError.value = "";
  if (session.pendingRemoteChanges) {
    logoutAlertOpen.value = true;
    return;
  }
  await doLogout(false);
}

async function doLogout(discard: boolean) {
  accountBusy.value = true;
  logoutAlertOpen.value = false;
  try {
    session.apply(await logoutAccount(discard));
    await refetchAccount();
    await queryClient.invalidateQueries({ queryKey: ["calendars"] });
    await queryClient.invalidateQueries({ queryKey: ["events"] });
    await queryClient.invalidateQueries({ queryKey: ["tasks"] });
  } catch (e) {
    accountError.value = reportInvokeError("退出登录", e);
  } finally {
    accountBusy.value = false;
  }
}

async function onSyncThenLogout() {
  accountBusy.value = true;
  accountError.value = "";
  logoutAlertOpen.value = false;
  try {
    await session.syncNow();
    if (!session.pendingRemoteChanges) {
      session.apply(await logoutAccount(false));
      await refetchAccount();
    }
  } catch (e) {
    accountError.value = reportInvokeError("同步后退出", e);
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

function onOpenChange(next: boolean) {
  if (!next) {
    cancel();
  }
}

const jieqiRows: { season: JieqiSeason; label: string }[] = [
  { season: "spring", label: "春" },
  { season: "summer", label: "夏" },
  { season: "autumn", label: "秋" },
  { season: "winter", label: "冬" },
];

const upcomingDaysModel = computed({
  get: () => String(draft.value.upcomingDays),
  set: (value: string) => {
    const n = Number(value) as UpcomingDays;
    if (n === 7 || n === 14 || n === 30) {
      draft.value.upcomingDays = n;
    }
  },
});
</script>

<template>
  <Dialog :open="settings.dialogOpen" @update:open="onOpenChange">
    <DialogContent
      class="flex h-[min(90vh,720px)] max-w-3xl flex-col gap-0 overflow-hidden p-0 sm:max-w-3xl"
      :show-close-button="false"
      @pointer-down-outside="preventOverlayDismiss"
      @interact-outside="preventOverlayDismiss"
    >
      <DialogHeader class="shrink-0 px-6 pt-5 pb-3">
        <DialogTitle>设置</DialogTitle>
      </DialogHeader>

      <div class="flex min-h-0 flex-1 border-t border-border">
        <nav class="flex w-[140px] shrink-0 flex-col gap-1 border-r border-border p-2">
          <Button
            v-for="item in navItems"
            :key="item.id"
            type="button"
            variant="ghost"
            class="justify-start"
            :class="page === item.id ? 'bg-primary/15 text-primary hover:bg-primary/15 hover:text-primary' : ''"
            @click="page = item.id"
          >
            {{ item.label }}
          </Button>
        </nav>

        <ScrollArea class="min-h-0 flex-1">
          <div class="space-y-5 p-5">
            <section v-if="page === 'account'" class="space-y-4">
              <p class="text-muted-foreground text-xs">
                当前日历本是唯一读写目标。离线可切换日历，不能新建远端集合。
              </p>

              <div class="space-y-2">
                <p class="text-muted-foreground text-sm">当前日历本</p>
                <RadioGroup
                  :model-value="session.currentCalendarId"
                  class="gap-2"
                  @update:model-value="onSelectCalendar(String($event))"
                >
                  <label
                    v-for="cal in session.calendars"
                    :key="cal.id"
                    class="flex items-center gap-2 text-sm"
                  >
                    <RadioGroupItem :value="cal.id" />
                    <span
                      class="size-3 shrink-0 rounded-full border border-border"
                      :style="cal.color ? { backgroundColor: cal.color } : undefined"
                    />
                    <span>{{ cal.displayName }}{{ cal.accountId ? "" : "（本地）" }}</span>
                  </label>
                </RadioGroup>
              </div>

              <div v-if="loggedIn" class="space-y-3 text-sm">
                <p class="text-muted-foreground">{{ accountSummary() }}</p>
                <div class="space-y-3 rounded-md border border-border p-3">
                  <p class="text-muted-foreground">新增远端日历本</p>
                  <div class="flex items-center gap-2">
                    <Input
                      v-model="newCalName"
                      class="min-w-0 flex-1"
                      placeholder="名称"
                      :disabled="!online || accountBusy"
                    />
                    <ColorSwatchInput v-model="newCalColor" aria-label="日历颜色" :disabled="!online || accountBusy" />
                  </div>
                  <label class="flex items-center gap-2">
                    <Checkbox v-model="migrateLocal" :disabled="!online" />
                    从本地迁移
                  </label>
                  <label v-if="migrateLocal" class="flex items-center gap-2">
                    <Checkbox v-model="clearLocalAfterMigrate" />
                    迁移后清空本地
                  </label>
                  <Button type="button" variant="outline" :disabled="!online || accountBusy" @click="onCreateCalendar">
                    <Loader2 v-if="accountBusy" class="animate-spin" />
                    新建
                  </Button>
                </div>
                <div class="space-y-2">
                  <Label for="account-new-password">更新密码</Label>
                  <Input id="account-new-password" v-model="newPassword" type="password" />
                </div>
                <Button
                  type="button"
                  variant="outline"
                  :disabled="accountBusy || !newPassword"
                  @click="onUpdatePassword"
                >
                  <Loader2 v-if="accountBusy" class="animate-spin" />
                  保存密码并重连
                </Button>
                <div class="flex flex-wrap gap-2">
                  <Button type="button" variant="outline" :disabled="accountBusy" @click="onRediscover">
                    <Loader2 v-if="accountBusy" class="animate-spin" />
                    重新发现
                  </Button>
                  <Button type="button" variant="outline" :disabled="accountBusy" @click="requestLogout">
                    退出登录
                  </Button>
                </div>
              </div>

              <div v-else class="space-y-3">
                <div class="space-y-1.5">
                  <Label for="account-server">服务器</Label>
                  <Input
                    id="account-server"
                    v-model="accountForm.serverUrl"
                    type="url"
                    placeholder="http://127.0.0.1:5232"
                    autocomplete="off"
                  />
                </div>
                <div class="space-y-1.5">
                  <Label for="account-username">用户名</Label>
                  <Input id="account-username" v-model="accountForm.username" autocomplete="username" />
                </div>
                <div class="space-y-1.5">
                  <Label for="account-password">密码</Label>
                  <Input
                    id="account-password"
                    v-model="accountForm.password"
                    type="password"
                    autocomplete="current-password"
                  />
                </div>
                <Button
                  type="button"
                  :disabled="accountBusy || !accountForm.username || !accountForm.password"
                  @click="onConnectAccount"
                >
                  <Loader2 v-if="accountBusy" class="animate-spin" />
                  {{ accountBusy ? "连接中" : "连接" }}
                </Button>
              </div>

              <p v-if="accountError" class="text-destructive max-h-32 overflow-auto whitespace-pre-wrap break-all text-sm">{{ accountError }}</p>
            </section>

            <section v-else-if="page === 'appearance'" class="space-y-5">
              <div>
                <p class="text-muted-foreground mb-2 text-sm">主题色</p>
                <div class="flex flex-wrap items-center gap-2">
                  <ColorSwatchInput v-model="draft.primaryColor" aria-label="主题色" />
                  <Button
                    v-for="preset in THEME_PRESETS"
                    :key="preset.id"
                    type="button"
                    variant="outline"
                    class="size-8 p-0"
                    :style="{ backgroundColor: preset.hex }"
                    :title="preset.label"
                    @click="setPrimary(preset.hex)"
                  />
                </div>
              </div>
              <div>
                <p class="text-muted-foreground mb-2 text-sm">节气字色</p>
                <div class="space-y-2">
                  <div v-for="row in jieqiRows" :key="row.season" class="flex items-center gap-3 text-sm">
                    <span class="text-muted-foreground w-6 shrink-0">{{ row.label }}</span>
                    <ColorSwatchInput
                      :model-value="draft.jieqiColors[row.season]"
                      :aria-label="`${row.label}节气色`"
                      @update:model-value="setJieqi(row.season, $event)"
                    />
                    <span class="text-muted-foreground font-mono text-xs">{{
                      draft.jieqiColors[row.season]
                    }}</span>
                  </div>
                </div>
                <Button type="button" variant="link" class="mt-5 h-auto px-0" @click="resetJieqiDraft">
                  恢复默认节气色
                </Button>
              </div>
            </section>

            <section v-else-if="page === 'calendar'" class="space-y-5">
              <div>
                <p class="text-muted-foreground mb-2 text-sm">一周从</p>
                <RadioGroup v-model="draft.weekStart" class="flex flex-row gap-4">
                  <label class="flex items-center gap-2 text-sm">
                    <RadioGroupItem value="monday" />
                    周一
                  </label>
                  <label class="flex items-center gap-2 text-sm">
                    <RadioGroupItem value="sunday" />
                    周日
                  </label>
                </RadioGroup>
              </div>
              <label class="flex items-center gap-2 text-sm">
                <Checkbox v-model="draft.showMinorFestivals" />
                显示次要节日
              </label>
              <label class="flex items-center gap-2 text-sm">
                <Checkbox v-model="draft.scrollSnapWeeks" />
                滚动贴合周边界
              </label>
            </section>

            <section v-else class="space-y-3">
              <p class="text-muted-foreground text-sm">「待进行」列表显示范围</p>
              <RadioGroup v-model="upcomingDaysModel" class="gap-2">
                <label v-for="option in UPCOMING_DAY_OPTIONS" :key="option" class="flex items-center gap-2 text-sm">
                  <RadioGroupItem :value="String(option)" />
                  {{ option }} 天
                </label>
              </RadioGroup>
            </section>
          </div>
        </ScrollArea>
      </div>

      <DialogFooter class="mx-0 mb-0 shrink-0">
        <Button type="button" variant="outline" @click="cancel">取消</Button>
        <Button type="button" @click="confirm">确定</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <AlertDialog v-model:open="logoutAlertOpen">
    <AlertDialogContent @pointer-down-outside="preventOverlayDismiss" @interact-outside="preventOverlayDismiss">
      <AlertDialogHeader>
        <AlertDialogTitle>有未同步的远端改动</AlertDialogTitle>
        <AlertDialogDescription>先同步，或放弃这些改动后退出。</AlertDialogDescription>
      </AlertDialogHeader>
      <AlertDialogFooter class="flex-col sm:flex-col sm:space-x-0">
        <Button type="button" :disabled="accountBusy" @click="onSyncThenLogout">先同步再退出</Button>
        <Button type="button" variant="destructive" :disabled="accountBusy" @click="doLogout(true)">
          放弃并退出
        </Button>
        <AlertDialogCancel>取消</AlertDialogCancel>
      </AlertDialogFooter>
    </AlertDialogContent>
  </AlertDialog>
</template>
