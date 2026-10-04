use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Entry {
    pub id: i64,
    pub date: String,
    pub note: String,
    pub author: String,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Photo {
    pub id: String,
    pub entry_id: i64,
    pub filename: String,
    pub original_name: String,
    pub mime: String,
    pub created_at: String,
    pub caption: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Comment {
    pub id: i64,
    pub entry_id: i64,
    pub author: String,
    pub content: String,
    pub sticker: String,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Milestone {
    pub id: i64,
    pub name: String,
    pub date: String,
    pub emoji: String,
    pub repeat_yearly: i64,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VoiceNote {
    pub id: String,
    pub entry_id: i64,
    pub filename: String,
    pub original_name: String,
    pub mime: String,
    pub duration_seconds: f64,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LetterVoiceNote {
    pub id: String,
    pub letter_id: i64,
    pub filename: String,
    pub original_name: String,
    pub mime: String,
    pub duration_seconds: f64,
    pub created_at: String,
}
#[derive(Debug, Clone, FromRow)]
pub struct LetterRow {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub author: String,
    pub open_at: String,
    pub created_at: String,
    pub opened_at: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct LetterResp {
    pub id: i64,
    pub title: String,
    pub author: String,
    pub open_at: String,
    pub created_at: String,
    pub opened_at: Option<String>,
    pub unlocked: bool,
    pub days_until: i64,
    pub content: Option<String>,
    pub voice_count: i64,
    pub voice_notes: Vec<LetterVoiceNote>,
}

#[derive(Debug, Serialize)]
pub struct EntryWithPhotos {
    #[serde(flatten)]
    pub entry: Entry,
    pub photos: Vec<Photo>,
}

#[derive(Debug, Serialize)]
pub struct EntryDetail {
    #[serde(flatten)]
    pub entry: Entry,
    pub photos: Vec<Photo>,
    pub comments: Vec<Comment>,
    pub voice_notes: Vec<VoiceNote>,
}

#[derive(Debug, Deserialize)]
pub struct LoginReq {
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResp {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateEntryReq {
    pub date: String,
    pub note: String,
    pub author: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEntryReq {
    pub date: String,
    pub note: String,
    pub author: String,
    pub version: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentReq {
    pub author: String,
    pub content: String,
    pub sticker: String,
}
#[derive(Debug, Deserialize)]
pub struct UpdatePhotoReq {
    pub caption: String,
}
#[derive(Debug, Deserialize)]
pub struct CreateMilestoneReq {
    pub name: String,
    pub date: String,
    pub emoji: String,
    pub repeat_yearly: i64,
}
#[derive(Debug, Deserialize)]
pub struct UpdateMilestoneReq {
    pub name: String,
    pub date: String,
    pub emoji: String,
    pub repeat_yearly: i64,
}
#[derive(Debug, Deserialize)]
pub struct CreateLetterReq {
    pub title: String,
    pub content: String,
    pub author: String,
    pub open_at: String,
}

#[derive(Debug, Serialize)]
pub struct StatsResp {
    pub days_together: i64,
    pub next_anniversary: Option<Anniversary>,
    pub love_start: String,
    pub entry_count: i64,
    pub photo_count: i64,
    pub comment_count: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Anniversary {
    pub name: String,
    pub date: String,
    pub days_until: i64,
}
#[derive(Debug, Serialize, FromRow)]
pub struct MonthlyReview {
    pub month: String,
    pub count: i64,
}
#[derive(Debug, Serialize)]
pub struct ReviewResp {
    pub days_together: i64,
    pub entry_count: i64,
    pub photo_count: i64,
    pub comment_count: i64,
    pub voice_count: i64,
    pub total_words: i64,
    pub first_entry: Option<Entry>,
    pub latest_entry: Option<Entry>,
    pub busiest_month: Option<String>,
    pub monthly: Vec<MonthlyReview>,
    pub recent_photos: Vec<Photo>,
}

pub fn parse_entry_date(value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "日期格式应为 YYYY-MM-DD".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_entry_dates() {
        assert!(parse_entry_date("2026-10-02").is_ok());
        assert!(parse_entry_date("2026-02-29").is_err());
        assert!(parse_entry_date("02/10/2026").is_err());
    }
}
