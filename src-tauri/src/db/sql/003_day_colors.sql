-- 月视图日格底色（本地装饰，不进 CalDAV / ICS）

CREATE TABLE day_colors (
    date TEXT PRIMARY KEY NOT NULL,
    color TEXT NOT NULL CHECK (color IN ('green', 'red'))
);
