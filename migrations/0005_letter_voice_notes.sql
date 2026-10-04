CREATE TABLE IF NOT EXISTS letter_voice_notes (
    id TEXT PRIMARY KEY,
    letter_id INTEGER NOT NULL,
    filename TEXT NOT NULL,
    original_name TEXT NOT NULL,
    mime TEXT NOT NULL,
    duration_seconds REAL NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    FOREIGN KEY(letter_id) REFERENCES letters(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_letter_voice_notes_letter ON letter_voice_notes(letter_id);
