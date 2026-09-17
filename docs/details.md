# 已敲定的技术细节

问卷与后续确认中已经拍板的内容。未写入本节的事项仍须再问，不可自行补全。

## 项目约定

- **未定先问**：除已写入本文档 / `plan.md` 的技术栈，以及你明确说可以定的方案外，拿不定或有多方案利弊时必须先问。未让自由发挥时，不可想当然（例如擅自起软件名却不说明这里自己做了主）。
- **文档同步**：有新的已定细节、你主动要求记录的 idea、或技术栈变动时，立刻写入对应文档。
  - `docs/details.md`：已拍板的技术 / 产品细节（实现后也把实际约定补进去）
  - `docs/idea.md`：只记你明确要求留下的构想，不自行发挥
  - `docs/plan.md`：技术方案与技术栈矩阵

## 项目身份

- 仓库根目录即最终工程根：前端与 Tauri/Rust 后端都在此目录下（`src/`、`src-tauri/`），不另拆仓库。
- 显示名 / 窗口标题：`FCalendar`
- npm / Cargo 包名：`f-calendar`
- Tauri identifier：`com.f.cal`
- 包管理器：`pnpm`
- 启动预览：`package.json` 的 `"tauri": "tauri dev"`，入口命令是 **`pnpm tauri`**（不要改成只转发 CLI，否则无子命令会直接退出）

## CalDAV

- 第一家对接：**Radicale**
- 凭据不进 SQLite，只存 `credential_ref`（具体钥匙串方案尚未定）

## 本地库

- SQLite 文件：应用数据目录下的 `fcalendar.db`（Windows 为 `%APPDATA%\com.f.cal\`）
- 已有数据时可以用 `ALTER TABLE ... ADD COLUMN` 加列；有行时不能加「无 DEFAULT 的 NOT NULL」。旧迁移不改，只追加新版本（`rusqlite_migration`）。
- `r2d2` 通过 **`r2d2_sqlite`** 连接 `rusqlite`（方案未写但技术上必需）。
- 表（`001_init`）：`accounts`、`calendars`、`events`、`todos`、`alarms`、`memos`、`change_queue`、`conflicts`。
- 纯本地日历：`calendars.account_id` 为空。
- 事件/待办：`ics` 为同步真源，其余列为查询投影；`(calendar_id, uid)` 唯一。
- 首次启动：若没有任何日历，预置 1 个本地日历，显示名「本地」，颜色留空。

## 初始验证预览的范围

目标：本地数据库可用 + 前端简单月视图，用来验证读写。

包含：

- 日历 + VEVENT 的本地 CRUD（Tauri command + Vue Query）
- Rust 用 `icalendar` 从结构化字段生成 ICS
- Schedule-X **月视图**
- 月视图内新建 / 编辑 / 删除

不包含：

- 周/日/Agenda 视图
- 拖拽改期、边缘拉伸
- 左侧 Tasks、拖任务上日历、恒定图章
- 待办 / 备忘 / 提醒的业务 CRUD
- 写 `change_queue`
- 网络同步与 Radicale 通信
- 地点、status 表单字段

## 事件写入约定

- 带时刻：ISO-8601 **含偏移**（如 `2026-09-17T10:00:00+08:00`）
- 全天：`YYYY-MM-DD`；ICS 的 `DTEND` 按 RFC 5545 开区间处理
- 表里暂无独立 `timezone` 列；偏移先保住绝对时间，夏令时 / TZID 以后再加列
- 可写 `rrule` 字符串并编进 ICS；**库内不展开实例**
- 仅当该事件所属日历的 `account_id` 非空时，本地写入才 `dirty=1`
- 本切片不写 `change_queue`（同步引擎再做）

## 重复规则（RRULE）

- 前端用**预设选项**（及以后的自定义面板）生成规则，不让用户手写 RRULE 字符串；后端仍收字符串。
- 月视图：按**可见月份窗口**在内存展开，不写回库。
- 点任意实例的编辑 / 删除都作用在**整组**（改主事件）。不做「仅此次」（无 EXDATE / 例外实例）。

## 月视图表单字段

- 要：标题、全天开关、开始、结束、重复、备注
- 不要：地点、status（`CONFIRMED` / `TENTATIVE` / `CANCELLED`）
- 标题必填；删除在对话框内且需确认
- 重复预设：不重复 / 每天 / 每周（同星期几） / 每月（同号） / 每年；无结束条件、无自定义 RRULE 面板
- 表单全天结束日按**含当日**填写；写入 ICS / 库时 `DTEND` 按 RFC 5545 开区间（+1 天）
- 点空白日 → 对话框新建（预填该日；默认非全天、结束 = 开始 + 1 小时）
- 点事件 → 同一对话框编辑
- 点任意重复实例，改/删整组

## 预览布局与窗口

- 窗口：`decorations: false` 自绘顶栏；默认 **1600×900**、可缩放；预览不做 Snap 预览、不做 Win11 圆角特化
- 顶栏可拖窗口（`data-tauri-drag-region`）；右上自绘最小化 / 最大化 / 关闭（Tauri `core:window` 权限）
- 三栏：固定 **80px** 左边栏（拖窗口）| 任务区默认 **360**、最小 **320**（本切片留空）| 右侧 Schedule-X 月视图
- 左边栏一个文本按钮收起/打开任务区
- 任务区与月视图之间分割线可拖：缩到 320 仍显示；**同一次拖动左移距离超过按下时任务区宽度的 2/3** 则隐藏
- 拖藏后按钮展开回到 **320**；点按钮收起则记住当时宽度，再展开恢复该宽度
- 月视图左上：中文年月份；其右：文本「上个月 / 回到当前月 / 下个月」；Schedule-X 默认 header 隐藏
- 顶栏拖动区不要盖住切月按钮和窗口三按钮（按钮组 `@pointerdown.stop`）
- 无边框窗口在 Windows 上 `isMaximized()` 不可靠：最大化前记住 inner size + outer position；图标 `□` / `❐` 用本地状态；还原时 `unmaximize` 后再按记住的尺寸设回去。额外权限：`unmaximize`、`is-maximized`、`inner-size`、`outer-position`、`set-size`、`set-position`、`current-monitor`
- 拖任务区分割线时禁止文本选中（`user-select: none` + 指针捕获）
- 周一到周日单独一行写在月网格**上方**，不写进第一周日期格子
- 月网格撑满顶栏与星期行之下的剩余高度
- 切月以顶栏年月为唯一源，不要用 Schedule-X `range.start`（那是格子第一天，常落在上月尾巴）回写当前月
- 一周从周一起；界面中文
- 仅月网格；切换月份按可见网格窗口重新 `list_events`；不做拖拽改期
- 带时刻事件交给 Schedule-X 时：`Temporal.Instant.from(含偏移 ISO)` → 系统 IANA 时区的 `ZonedDateTime`；全天用 `PlainDate`，结束日按库返回的**含当日**
- Schedule-X 事件 `id` 只能是 `[A-Za-z0-9_-]`

## 预览 Tauri Commands

- `list_calendars`（内部用，界面不展示切换）
- `list_events(from, to, calendarId?)`：过滤软删，返回展开后的实例 + 主事件 id；实例 id = `{主事件id}_{去掉:-+:T 的 dtstart}`（不用冒号，以便 Schedule-X）
- `get_event` / `create_event` / `update_event` / `delete_event`（软删 `deleted_at`）
- 预览本地日历无 `account_id`，`dirty` 恒为 0；不写 `change_queue`

## 初始化时已定、与预览无冲突的宿主细节

- 托盘：Tauri 2 核心 `tray-icon`（`tauri` crate feature），无独立 `plugin-tray`
- Schedule-X 为官方 **v4**（`@schedule-x/calendar` / `theme-default` / `vue`）+ `temporal-polyfill@0.3.0`；`date-fns` 仍安装供业务日期使用
- 前端日历时区：`Intl` 解析出的系统 IANA 名，传给 Schedule-X `timezone`
- shadcn-vue：按当时官方 CLI 默认（Tailwind **v4**；实际初始化为 `reka-nova` + Neutral）
- Rust 另用 `uuid`（v4）、`chrono`（带偏移时间 / 全天日期）；连接池为 `r2d2` + `r2d2_sqlite`
- 开机自启插件只注册，不默认 enable
- 单实例插件 callback 为空
