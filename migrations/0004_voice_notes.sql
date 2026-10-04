CREATE TABLE IF NOT EXISTS voice_notes (
    id TEXT PRIMARY KEY,
    entry_id INTEGER NOT NULL,
    filename TEXT NOT NULL,
    original_name TEXT NOT NULL,
    mime TEXT NOT NULL,
    duration_seconds REAL NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    FOREIGN KEY(entry_id) REFERENCES entries(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_voice_notes_entry ON voice_notes(entry_id);
