ALTER TABLE photos ADD COLUMN caption TEXT NOT NULL DEFAULT '';

CREATE TABLE IF NOT EXISTS milestones (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    date TEXT NOT NULL,
    emoji TEXT NOT NULL DEFAULT '',
    repeat_yearly INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_milestones_date ON milestones(date);
