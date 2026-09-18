-- Local inbox tasks and stamps (not CalDAV / not VTODO).

CREATE TABLE tasks (
    id TEXT PRIMARY KEY NOT NULL,
    summary TEXT NOT NULL,
    description TEXT,
    is_stamp INTEGER NOT NULL DEFAULT 0 CHECK (is_stamp IN (0, 1)),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_tasks_stamp_sort ON tasks (is_stamp, sort_order);
