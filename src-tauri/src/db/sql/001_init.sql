-- Local-First schema. Sync-related columns are reserved for the later CalDAV
-- (Radicale first) engine; this migration only creates storage.

CREATE TABLE accounts (
    id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    server_url TEXT NOT NULL,
    username TEXT NOT NULL,
    -- OS credential-store key. Never store the password / token here.
    credential_ref TEXT NOT NULL,
    principal_url TEXT,
    calendar_home_url TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE (server_url, username)
);

CREATE TABLE calendars (
    id TEXT PRIMARY KEY NOT NULL,
    -- NULL = local-only collection (no CalDAV account yet).
    account_id TEXT REFERENCES accounts (id) ON DELETE CASCADE,
    href TEXT NOT NULL,
    display_name TEXT NOT NULL,
    color TEXT,
    description TEXT,
    timezone TEXT,
    ctag TEXT,
    sync_token TEXT,
    supports_vevent INTEGER NOT NULL DEFAULT 1 CHECK (supports_vevent IN (0, 1)),
    supports_vtodo INTEGER NOT NULL DEFAULT 0 CHECK (supports_vtodo IN (0, 1)),
    visible INTEGER NOT NULL DEFAULT 1 CHECK (visible IN (0, 1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX idx_calendars_account_href
    ON calendars (account_id, href)
    WHERE account_id IS NOT NULL;

CREATE INDEX idx_calendars_account ON calendars (account_id);

CREATE TABLE events (
    id TEXT PRIMARY KEY NOT NULL,
    calendar_id TEXT NOT NULL REFERENCES calendars (id) ON DELETE CASCADE,
    uid TEXT NOT NULL,
    href TEXT,
    etag TEXT,
    -- Full iCalendar payload; structured columns are query projections.
    ics TEXT NOT NULL,
    summary TEXT,
    description TEXT,
    location TEXT,
    dtstart TEXT NOT NULL,
    dtend TEXT,
    all_day INTEGER NOT NULL DEFAULT 0 CHECK (all_day IN (0, 1)),
    rrule TEXT,
    status TEXT,
    dirty INTEGER NOT NULL DEFAULT 0 CHECK (dirty IN (0, 1)),
    deleted_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE (calendar_id, uid)
);

CREATE INDEX idx_events_calendar_dtstart ON events (calendar_id, dtstart);
CREATE INDEX idx_events_dirty ON events (dirty) WHERE dirty = 1 AND deleted_at IS NULL;

CREATE TABLE todos (
    id TEXT PRIMARY KEY NOT NULL,
    calendar_id TEXT NOT NULL REFERENCES calendars (id) ON DELETE CASCADE,
    uid TEXT NOT NULL,
    href TEXT,
    etag TEXT,
    ics TEXT NOT NULL,
    summary TEXT,
    description TEXT,
    due TEXT,
    completed_at TEXT,
    percent INTEGER,
    priority INTEGER,
    status TEXT,
    dirty INTEGER NOT NULL DEFAULT 0 CHECK (dirty IN (0, 1)),
    deleted_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE (calendar_id, uid)
);

CREATE INDEX idx_todos_calendar_due ON todos (calendar_id, due);
CREATE INDEX idx_todos_dirty ON todos (dirty) WHERE dirty = 1 AND deleted_at IS NULL;

-- Expanded local reminders. Parent is event or todo; no FK so a tombstone
-- can keep the alarm until the change queue is flushed.
CREATE TABLE alarms (
    id TEXT PRIMARY KEY NOT NULL,
    parent_type TEXT NOT NULL CHECK (parent_type IN ('event', 'todo')),
    parent_id TEXT NOT NULL,
    trigger_at INTEGER NOT NULL,
    trigger_ics TEXT,
    action TEXT NOT NULL DEFAULT 'DISPLAY',
    description TEXT,
    fired_at INTEGER,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_alarms_due ON alarms (trigger_at) WHERE fired_at IS NULL;
CREATE INDEX idx_alarms_parent ON alarms (parent_type, parent_id);

CREATE TABLE memos (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_memos_updated ON memos (updated_at DESC);

-- Offline mutation queue. Memos are local-only and never enqueued.
CREATE TABLE change_queue (
    id TEXT PRIMARY KEY NOT NULL,
    entity_type TEXT NOT NULL CHECK (entity_type IN ('calendar', 'event', 'todo')),
    entity_id TEXT NOT NULL,
    op TEXT NOT NULL CHECK (op IN ('create', 'update', 'delete')),
    payload TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'in_flight', 'failed')),
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_change_queue_status ON change_queue (status, created_at);

-- Remote kept as-is; local edit saved as a copy (see plan conflict policy).
CREATE TABLE conflicts (
    id TEXT PRIMARY KEY NOT NULL,
    entity_type TEXT NOT NULL CHECK (entity_type IN ('event', 'todo')),
    remote_id TEXT NOT NULL,
    local_copy_id TEXT NOT NULL,
    remote_etag TEXT,
    note TEXT,
    created_at INTEGER NOT NULL
);
