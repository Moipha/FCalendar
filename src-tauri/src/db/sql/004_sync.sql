-- 当前日历本同步：任务/日格颜色挂 calendar_id；应用状态；队列类型扩到 task / day_color。

ALTER TABLE tasks ADD COLUMN calendar_id TEXT REFERENCES calendars (id) ON DELETE CASCADE;
ALTER TABLE tasks ADD COLUMN uid TEXT;
ALTER TABLE tasks ADD COLUMN href TEXT;
ALTER TABLE tasks ADD COLUMN etag TEXT;
ALTER TABLE tasks ADD COLUMN ics TEXT;
ALTER TABLE tasks ADD COLUMN dirty INTEGER NOT NULL DEFAULT 0 CHECK (dirty IN (0, 1));
ALTER TABLE tasks ADD COLUMN deleted_at INTEGER;

UPDATE tasks
SET calendar_id = (SELECT id FROM calendars WHERE account_id IS NULL ORDER BY created_at ASC LIMIT 1)
WHERE calendar_id IS NULL;

UPDATE tasks SET uid = id WHERE uid IS NULL;

CREATE TABLE day_colors_v2 (
    calendar_id TEXT NOT NULL REFERENCES calendars (id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    color TEXT NOT NULL CHECK (color IN ('green', 'red')),
    uid TEXT NOT NULL,
    href TEXT,
    etag TEXT,
    ics TEXT,
    dirty INTEGER NOT NULL DEFAULT 0 CHECK (dirty IN (0, 1)),
    deleted_at INTEGER,
    PRIMARY KEY (calendar_id, date)
);

INSERT INTO day_colors_v2 (calendar_id, date, color, uid, dirty)
SELECT
    (SELECT id FROM calendars WHERE account_id IS NULL ORDER BY created_at ASC LIMIT 1),
    date,
    color,
    'fc-daycolor-' || date,
    0
FROM day_colors;

DROP TABLE day_colors;
ALTER TABLE day_colors_v2 RENAME TO day_colors;

CREATE INDEX idx_day_colors_cal_date ON day_colors (calendar_id, date);
CREATE INDEX idx_tasks_calendar ON tasks (calendar_id, is_stamp, sort_order);
CREATE INDEX idx_tasks_dirty ON tasks (dirty) WHERE dirty = 1 AND deleted_at IS NULL;

CREATE TABLE change_queue_v2 (
    id TEXT PRIMARY KEY NOT NULL,
    entity_type TEXT NOT NULL CHECK (entity_type IN ('calendar', 'event', 'todo', 'task', 'day_color')),
    entity_id TEXT NOT NULL,
    op TEXT NOT NULL CHECK (op IN ('create', 'update', 'delete')),
    payload TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'in_flight', 'failed')),
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

INSERT INTO change_queue_v2 SELECT * FROM change_queue;
DROP TABLE change_queue;
ALTER TABLE change_queue_v2 RENAME TO change_queue;
CREATE INDEX idx_change_queue_status ON change_queue (status, created_at);

CREATE TABLE app_state (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
