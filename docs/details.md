# 已敲定的技术细节

问卷与后续确认中已经拍板的内容。未写入本节的事项仍须再问，不可自行补全。

## 项目身份

- 仓库根目录即最终工程根：前端与 Tauri/Rust 后端都在此目录下（`src/`、`src-tauri/`），不另拆仓库。
- 显示名 / 窗口标题：`FCalendar`
- npm / Cargo 包名：`f-calendar`
- Tauri identifier：`com.f.cal`
- 包管理器：`pnpm`

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

## 初始化时已定、与预览无冲突的宿主细节

- 托盘：Tauri 2 核心 `tray-icon`，无独立 `plugin-tray`
- Schedule-X 走当前官方 v3 + `temporal-polyfill`（对齐 peer `0.3.x`）；`date-fns` 仍安装供业务日期使用
- shadcn-vue：按当时官方 CLI 默认（Tailwind v4；实际初始化为 `reka-nova` + Neutral）
- 开机自启插件只注册，不默认 enable
- 单实例插件 callback 为空
