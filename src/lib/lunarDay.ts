import { SolarDay } from "tyme4ts";

export type CornerBadgeKind = "major" | "jieqi" | "minor";

export type JieqiSeason = "spring" | "summer" | "autumn" | "winter";

export type CornerBadge = {
  text: string;
  kind: CornerBadgeKind;
  season?: JieqiSeason;
};

export type WorkMark = "off" | "work" | null;

export type LunarDayInfo = {
  lunarText: string;
  cornerBadges: CornerBadge[];
  workMark: WorkMark;
};

/** 右下角名称优先级 0：重大传统 / 法定节日 */
const MAJOR_FESTIVAL_NAMES = new Set([
  "除夕",
  "春节",
  "元宵节",
  "清明节",
  "端午节",
  "七夕节",
  "中秋节",
  "重阳节",
  "腊八节",
  "小年",
  "元旦",
  "妇女节",
  "劳动节",
  "青年节",
  "儿童节",
  "建党节",
  "建军节",
  "教师节",
  "国庆节",
]);

const BADGE_KIND_RANK: Record<CornerBadgeKind, number> = {
  major: 0,
  jieqi: 1,
  minor: 2,
};

const JIEQI_SEASON: Record<string, JieqiSeason> = {
  立春: "spring",
  雨水: "spring",
  惊蛰: "spring",
  春分: "spring",
  清明: "spring",
  谷雨: "spring",
  立夏: "summer",
  小满: "summer",
  芒种: "summer",
  夏至: "summer",
  小暑: "summer",
  大暑: "summer",
  立秋: "autumn",
  处暑: "autumn",
  白露: "autumn",
  秋分: "autumn",
  寒露: "autumn",
  霜降: "autumn",
  立冬: "winter",
  小雪: "winter",
  大雪: "winter",
  冬至: "winter",
  小寒: "winter",
  大寒: "winter",
};

function festivalKind(name: string): CornerBadgeKind {
  return MAJOR_FESTIVAL_NAMES.has(name) ? "major" : "minor";
}

/** 「清明节」相对「清明」这类节气同名节日，只保留节气。 */
function isFestivalAliasOfJieqi(festival: string, jieqi: string) {
  return festival === jieqi || festival === `${jieqi}节` || jieqi === `${festival}节`;
}

function collectCornerCandidates(solar: SolarDay): CornerBadge[] {
  const candidates: CornerBadge[] = [];

  const solarFestival = solar.getFestival();
  if (solarFestival) {
    candidates.push({
      text: solarFestival.getName(),
      kind: festivalKind(solarFestival.getName()),
    });
  }

  const lunarFestival = solar.getLunarDay().getFestival();
  if (lunarFestival) {
    candidates.push({
      text: lunarFestival.getName(),
      kind: festivalKind(lunarFestival.getName()),
    });
  }

  const termDay = solar.getTermDay();
  if (termDay.getDayIndex() === 0) {
    const termName = solar.getTerm().getName();
    candidates.push({
      text: termName,
      kind: "jieqi",
      season: JIEQI_SEASON[termName],
    });
  }

  return candidates;
}

function pickCornerBadges(candidates: CornerBadge[]): CornerBadge[] {
  const jieqiNames = candidates.filter((item) => item.kind === "jieqi").map((item) => item.text);
  const withoutJieqiAliases = candidates.filter((item) => {
    if (item.kind === "jieqi") {
      return true;
    }
    return !jieqiNames.some((jieqi) => isFestivalAliasOfJieqi(item.text, jieqi));
  });

  const seen = new Set<string>();
  const unique: CornerBadge[] = [];
  for (const candidate of withoutJieqiAliases) {
    if (seen.has(candidate.text)) {
      continue;
    }
    seen.add(candidate.text);
    unique.push(candidate);
  }

  // 截取后再把节气放到右侧
  return unique
    .sort((left, right) => BADGE_KIND_RANK[left.kind] - BADGE_KIND_RANK[right.kind])
    .slice(0, 2)
    .sort((left, right) => Number(left.kind === "jieqi") - Number(right.kind === "jieqi"));
}

function lunarDisplayText(solar: SolarDay): string {
  const lunarDay = solar.getLunarDay();
  if (lunarDay.getDay() === 1) {
    return lunarDay.getLunarMonth().getName();
  }
  return lunarDay.getName();
}

function workMarkFromLegal(solar: SolarDay): WorkMark {
  const legal = solar.getLegalHoliday();
  if (!legal) {
    return null;
  }
  return legal.isWork() ? "work" : "off";
}

export function getLunarDayInfo(dateIso: string): LunarDayInfo {
  const [year, month, day] = dateIso.split("-").map(Number);
  const solar = SolarDay.fromYmd(year, month, day);

  return {
    lunarText: lunarDisplayText(solar),
    cornerBadges: pickCornerBadges(collectCornerCandidates(solar)),
    workMark: workMarkFromLegal(solar),
  };
}

export function buildLunarDayMap(dateKeys: string[]): Map<string, LunarDayInfo> {
  const map = new Map<string, LunarDayInfo>();
  for (const dateKey of dateKeys) {
    map.set(dateKey, getLunarDayInfo(dateKey));
  }
  return map;
}
