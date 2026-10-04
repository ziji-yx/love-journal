CREATE TABLE IF NOT EXISTS letters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    author TEXT NOT NULL,
    open_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    opened_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_letters_open_at ON letters(open_at);
