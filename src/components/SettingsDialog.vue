<script setup lang="ts">
import { ref, watch } from "vue";

import { DEFAULT_JIEQI_COLORS, THEME_PRESETS, type JieqiSeason } from "@/lib/settingsTheme";
import {
  UPCOMING_DAY_OPTIONS,
  useSettingsStore,
  type AppSettings,
  type UpcomingDays,
} from "@/stores/settings";

const settings = useSettingsStore();

type Draft = AppSettings;

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
