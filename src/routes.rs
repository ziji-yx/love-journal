use std::{collections::HashMap, path::PathBuf};

use axum::{
    body::{Body, Bytes},
    extract::{Multipart, Path, Query, State},
    http::{
        header::{CACHE_CONTROL, CONTENT_TYPE, SET_COOKIE},
        HeaderMap, HeaderValue,
    },
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Datelike, NaiveDate, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::{auth, error::AppError, models::*, AppState};

pub const MAX_PHOTO_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_PHOTOS_PER_REQUEST: usize = 8;
pub const MAX_UPLOAD_BODY_BYTES: usize = (MAX_PHOTO_BYTES + 1024 * 1024) * MAX_PHOTOS_PER_REQUEST;

#[derive(Debug, Deserialize)]
pub struct EntriesQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub q: Option<String>,
}

struct PendingPhoto {
    id: String,
    entry_id: i64,
    filename: String,
    original_name: String,
    mime: String,
    created_at: String,
    data: Vec<u8>,
}

fn ensure_text(field: &str, value: &str, min: usize, max: usize) -> Result<(), AppError> {
    let length = value.chars().count();
    if length < min || length > max {
        return Err(AppError::BadRequest(format!(
            "{field} 长度应在 {min} 到 {max} 个字符之间"
        )));
    }
    Ok(())
}

fn validate_entry(req: &CreateEntryReq) -> Result<(), AppError> {
    parse_entry_date(&req.date).map_err(AppError::BadRequest)?;
    ensure_text("内容", &req.note, 1, 10_000)?;
    ensure_text("作者", &req.author, 1, 20)?;
    Ok(())
}

fn validate_update(req: &UpdateEntryReq) -> Result<(), AppError> {
    parse_entry_date(&req.date).map_err(AppError::BadRequest)?;
    ensure_text("内容", &req.note, 1, 10_000)?;
    ensure_text("作者", &req.author, 1, 20)?;
    Ok(())
}
fn validate_milestone(name: &str, date: &str, emoji: &str) -> Result<(), AppError> {
    ensure_text("名称", name, 1, 40)?;
    parse_entry_date(date).map_err(AppError::BadRequest)?;
    ensure_text("图标", emoji, 0, 16)?;
    Ok(())
}

fn validate_range(query: &EntriesQuery) -> Result<(), AppError> {
    let from = query
        .from
        .as_deref()
        .map(parse_entry_date)
        .transpose()
        .map_err(AppError::BadRequest)?;
    let to = query
        .to
        .as_deref()
        .map(parse_entry_date)
        .transpose()
        .map_err(AppError::BadRequest)?;

    if let (Some(from), Some(to)) = (from, to) {
        if from > to {
            return Err(AppError::BadRequest("起始日期不能晚于结束日期".into()));
        }
    }

    Ok(())
}

fn next_anniversary(today: NaiveDate, love_start: NaiveDate) -> Option<Anniversary> {
    for years in 0..=100 {
        let target_year = love_start.year() + years;
        let candidate = NaiveDate::from_ymd_opt(target_year, love_start.month(), love_start.day())
            .or_else(|| {
                if love_start.month() == 2 && love_start.day() == 29 {
                    NaiveDate::from_ymd_opt(target_year, 2, 28)
                } else {
                    None
                }
            })?;

        if candidate >= today {
            return Some(Anniversary {
                name: if years == 0 {
                    "在一起".to_string()
                } else {
                    format!("{years}周年")
                },
                date: candidate.format("%Y-%m-%d").to_string(),
                days_until: (candidate - today).num_days(),
            });
        }
    }

    None
}

fn detect_image(data: &[u8]) -> Option<(&'static str, &'static str)> {
    if data.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(("jpg", "image/jpeg"));
    }
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some(("png", "image/png"));
    }
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        return Some(("gif", "image/gif"));
    }
    if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        return Some(("webp", "image/webp"));
    }
    None
}

fn is_safe_filename(filename: &str) -> bool {
    !filename.is_empty()
        && !filename.contains('/')
        && !filename.contains('\\')
        && !filename.contains("..")
}

async fn remove_files(paths: &[PathBuf]) {
    for path in paths {
        let _ = tokio::fs::remove_file(path).await;
    }
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginReq>,
) -> Result<Response, AppError> {
    let token = auth::login(&state, &req.password).await?;
    let mut response = Json(LoginResp {
        token: token.clone(),
    })
    .into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        auth::session_cookie(&token, state.cookie_secure),
    );
    Ok(response)
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    auth::logout(&state, &headers).await?;
    let mut response = Json(serde_json::json!({ "ok": true })).into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, auth::clear_session_cookie(state.cookie_secure));
    Ok(response)
}

pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    auth::check(&state, &headers).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
pub async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

pub async fn stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<StatsResp>, AppError> {
    auth::check(&state, &headers).await?;

    let today = chrono::Local::now().date_naive();
    let days_together = (today - state.love_start).num_days().max(0);

    let entry_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entries")
        .fetch_one(&state.db)
        .await?;
    let photo_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM photos")
        .fetch_one(&state.db)
        .await?;
    let comment_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM comments")
        .fetch_one(&state.db)
        .await?;

    Ok(Json(StatsResp {
        days_together,
        next_anniversary: next_anniversary(today, state.love_start),
        love_start: state.love_start.format("%Y-%m-%d").to_string(),
        entry_count,
        photo_count,
        comment_count,
    }))
}

pub async fn random_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Option<EntryWithPhotos>>, AppError> {
    auth::check(&state, &headers).await?;

    let entry: Option<Entry> =
        sqlx::query_as::<_, Entry>("SELECT * FROM entries ORDER BY RANDOM() LIMIT 1")
            .fetch_optional(&state.db)
            .await?;

    if let Some(entry) = entry {
        let photos = sqlx::query_as::<_, Photo>(
            "SELECT * FROM photos WHERE entry_id = ? ORDER BY created_at",
        )
        .bind(entry.id)
        .fetch_all(&state.db)
        .await?;
        Ok(Json(Some(EntryWithPhotos { entry, photos })))
    } else {
        Ok(Json(None))
    }
}

pub async fn list_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<EntriesQuery>,
) -> Result<Json<Vec<EntryWithPhotos>>, AppError> {
    auth::check(&state, &headers).await?;
    validate_range(&query)?;

    let mut sql = String::from("SELECT * FROM entries");
    let mut conditions = Vec::new();

    if query.from.is_some() {
        conditions.push("date >= ?");
    }
    if query.to.is_some() {
        conditions.push("date <= ?");
    }
    let search = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(value) = search {
        if value.chars().count() > 100 {
            return Err(AppError::BadRequest("搜索内容不能超过 100 个字符".into()));
        }
    }
    let search_pattern = search.map(|value| format!("%{value}%"));
    if search_pattern.is_some() {
        conditions.push("(note LIKE ? OR date LIKE ? OR author LIKE ?)");
    }
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(" ORDER BY date DESC, id DESC");

    let mut db_query = sqlx::query_as::<_, Entry>(&sql);
    if let Some(from) = &query.from {
        db_query = db_query.bind(from);
    }
    if let Some(to) = &query.to {
        db_query = db_query.bind(to);
    }
    if let Some(pattern) = &search_pattern {
        db_query = db_query.bind(pattern).bind(pattern).bind(pattern);
    }

    let entries = db_query.fetch_all(&state.db).await?;
    let mut photos_by_entry: HashMap<i64, Vec<Photo>> = HashMap::new();

    if !entries.is_empty() {
        let placeholders = vec!["?"; entries.len()].join(",");
        let photos_sql = format!(
            "SELECT * FROM photos WHERE entry_id IN ({placeholders}) ORDER BY entry_id, created_at"
        );
        let mut photos_query = sqlx::query_as::<_, Photo>(&photos_sql);
        for entry in &entries {
            photos_query = photos_query.bind(entry.id);
        }

        for photo in photos_query.fetch_all(&state.db).await? {
            photos_by_entry
                .entry(photo.entry_id)
                .or_default()
                .push(photo);
        }
    }

    let result = entries
        .into_iter()
        .map(|entry| EntryWithPhotos {
            photos: photos_by_entry.remove(&entry.id).unwrap_or_default(),
            entry,
        })
        .collect();

    Ok(Json(result))
}

pub async fn get_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<EntryDetail>, AppError> {
    auth::check(&state, &headers).await?;

    let entry: Entry = sqlx::query_as("SELECT * FROM entries WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let photos =
        sqlx::query_as::<_, Photo>("SELECT * FROM photos WHERE entry_id = ? ORDER BY created_at")
            .bind(id)
            .fetch_all(&state.db)
            .await?;

    let comments = sqlx::query_as::<_, Comment>(
        "SELECT * FROM comments WHERE entry_id = ? ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(EntryDetail {
        entry,
        photos,
        comments,
    }))
}

pub async fn create_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateEntryReq>,
) -> Result<Json<Entry>, AppError> {
    auth::check(&state, &headers).await?;
    validate_entry(&req)?;

    let now = Utc::now().to_rfc3339();
    let entry: Entry = sqlx::query_as(
        "INSERT INTO entries (date, note, author, version, created_at, updated_at)
         VALUES (?, ?, ?, 1, ?, ?)
         RETURNING *",
    )
    .bind(&req.date)
    .bind(&req.note)
    .bind(&req.author)
    .bind(&now)
    .bind(&now)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(entry))
}

pub async fn update_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(req): Json<UpdateEntryReq>,
) -> Result<Json<Entry>, AppError> {
    auth::check(&state, &headers).await?;
    validate_update(&req)?;

    let mut transaction = state.db.begin().await?;
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE entries
         SET date = ?, note = ?, author = ?, version = version + 1, updated_at = ?
         WHERE id = ? AND version = ?",
    )
    .bind(&req.date)
    .bind(&req.note)
    .bind(&req.author)
    .bind(&now)
    .bind(id)
    .bind(req.version)
    .execute(&mut *transaction)
    .await?;

    if result.rows_affected() == 0 {
        let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM entries WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await?;

        return if exists.is_none() {
            Err(AppError::NotFound)
        } else {
            Err(AppError::Conflict(
                "对方刚刚修改过这篇手账，请刷新后再试".into(),
            ))
        };
    }

    let entry: Entry = sqlx::query_as("SELECT * FROM entries WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
    transaction.commit().await?;

    Ok(Json(entry))
}

pub async fn delete_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth::check(&state, &headers).await?;

    let photos = sqlx::query_as::<_, Photo>("SELECT * FROM photos WHERE entry_id = ?")
        .bind(id)
        .fetch_all(&state.db)
        .await?;

    let mut transaction = state.db.begin().await?;
    let result = sqlx::query("DELETE FROM entries WHERE id = ?")
        .bind(id)
        .execute(&mut *transaction)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    transaction.commit().await?;

    let paths: Vec<PathBuf> = photos
        .iter()
        .map(|photo| state.upload_dir.join(&photo.filename))
        .collect();
    remove_files(&paths).await;

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn upload_photos(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(entry_id): Path<i64>,
    mut multipart: Multipart,
) -> Result<Json<Vec<Photo>>, AppError> {
    auth::check(&state, &headers).await?;

    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM entries WHERE id = ?")
        .bind(entry_id)
        .fetch_optional(&state.db)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    let now = Utc::now().to_rfc3339();
    let mut pending = Vec::new();

    while let Some(field) = multipart.next_field().await? {
        if field.name().unwrap_or("") != "photos" {
            continue;
        }
        if pending.len() >= MAX_PHOTOS_PER_REQUEST {
            return Err(AppError::BadRequest(format!(
                "一次最多上传 {MAX_PHOTOS_PER_REQUEST} 张照片"
            )));
        }

        let original_name = field
            .file_name()
            .unwrap_or("photo")
            .chars()
            .take(255)
            .collect();
        let data = field.bytes().await?.to_vec();
        if data.is_empty() {
            return Err(AppError::BadRequest("不能上传空文件".into()));
        }
        if data.len() > MAX_PHOTO_BYTES {
            return Err(AppError::BadRequest(format!(
                "单张照片不能超过 {} MB",
                MAX_PHOTO_BYTES / 1024 / 1024
            )));
        }

        let Some((extension, mime)) = detect_image(&data) else {
            return Err(AppError::BadRequest(
                "仅支持 JPEG、PNG、GIF 和 WebP 图片".into(),
            ));
        };

        let id = Uuid::new_v4().to_string();
        pending.push(PendingPhoto {
            filename: format!("{id}.{extension}"),
            id,
            entry_id,
            original_name,
            mime: mime.to_string(),
            created_at: now.clone(),
            data,
        });
    }

    if pending.is_empty() {
        return Err(AppError::BadRequest("请选择要上传的照片".into()));
    }

    let mut written_paths = Vec::with_capacity(pending.len());
    for photo in &pending {
        let path = state.upload_dir.join(&photo.filename);
        if let Err(error) = tokio::fs::write(&path, &photo.data).await {
            tracing::error!("failed to write upload {:?}: {error}", path);
            remove_files(&written_paths).await;
            return Err(AppError::Internal("照片写入失败".into()));
        }
        written_paths.push(path);
    }

    let mut transaction = state.db.begin().await?;
    let mut photos = Vec::with_capacity(pending.len());

    for photo in &pending {
        let result = sqlx::query_as::<_, Photo>(
            "INSERT INTO photos (id, entry_id, filename, original_name, mime, created_at)
             VALUES (?, ?, ?, ?, ?, ?)
             RETURNING *",
        )
        .bind(&photo.id)
        .bind(photo.entry_id)
        .bind(&photo.filename)
        .bind(&photo.original_name)
        .bind(&photo.mime)
        .bind(&photo.created_at)
        .fetch_one(&mut *transaction)
        .await;

        match result {
            Ok(photo) => photos.push(photo),
            Err(error) => {
                drop(transaction);
                remove_files(&written_paths).await;
                return Err(error.into());
            }
        }
    }

    transaction.commit().await?;
    Ok(Json(photos))
}

pub async fn serve_upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(filename): Path<String>,
) -> Result<Response, AppError> {
    auth::check(&state, &headers).await?;

    if !is_safe_filename(&filename) {
        return Err(AppError::BadRequest("无效的照片文件名".into()));
    }

    let photo: Photo = sqlx::query_as("SELECT * FROM photos WHERE filename = ?")
        .bind(&filename)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let path = state.upload_dir.join(&photo.filename);
    let data = tokio::fs::read(&path).await.map_err(|error| {
        tracing::error!("failed to read upload {:?}: {error}", path);
        AppError::NotFound
    })?;

    let mime = HeaderValue::from_str(&photo.mime)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    let mut response = Response::new(Body::from(Bytes::from(data)));
    response.headers_mut().insert(CONTENT_TYPE, mime);
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=3600"),
    );
    Ok(response)
}

pub async fn delete_photo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth::check(&state, &headers).await?;

    let photo: Photo = sqlx::query_as("SELECT * FROM photos WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    sqlx::query("DELETE FROM photos WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;

    let path = state.upload_dir.join(&photo.filename);
    let _ = tokio::fs::remove_file(path).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}
pub async fn update_photo_caption(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdatePhotoReq>,
) -> Result<Json<Photo>, AppError> {
    auth::check(&state, &headers).await?;
    ensure_text("照片说明", &req.caption, 0, 200)?;

    let photo: Photo = sqlx::query_as("UPDATE photos SET caption = ? WHERE id = ? RETURNING *")
        .bind(req.caption.trim())
        .bind(&id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(photo))
}

pub async fn list_comments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(entry_id): Path<i64>,
) -> Result<Json<Vec<Comment>>, AppError> {
    auth::check(&state, &headers).await?;

    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM entries WHERE id = ?")
        .bind(entry_id)
        .fetch_optional(&state.db)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    let comments = sqlx::query_as::<_, Comment>(
        "SELECT * FROM comments WHERE entry_id = ? ORDER BY created_at",
    )
    .bind(entry_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(comments))
}

pub async fn create_comment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(entry_id): Path<i64>,
    Json(req): Json<CreateCommentReq>,
) -> Result<Json<Comment>, AppError> {
    auth::check(&state, &headers).await?;
    ensure_text("留言", &req.content, 1, 500)?;
    ensure_text("名字", &req.author, 1, 20)?;
    ensure_text("贴纸", &req.sticker, 0, 32)?;

    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM entries WHERE id = ?")
        .bind(entry_id)
        .fetch_optional(&state.db)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    let now = Utc::now().to_rfc3339();
    let comment: Comment = sqlx::query_as(
        "INSERT INTO comments (entry_id, author, content, sticker, created_at)
         VALUES (?, ?, ?, ?, ?)
         RETURNING *",
    )
    .bind(entry_id)
    .bind(&req.author)
    .bind(&req.content)
    .bind(&req.sticker)
    .bind(&now)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(comment))
}
pub async fn delete_comment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth::check(&state, &headers).await?;

    let result = sqlx::query("DELETE FROM comments WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn list_milestones(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Milestone>>, AppError> {
    auth::check(&state, &headers).await?;
    let milestones = sqlx::query_as::<_, Milestone>("SELECT * FROM milestones ORDER BY date, id")
        .fetch_all(&state.db)
        .await?;
    Ok(Json(milestones))
}

pub async fn create_milestone(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateMilestoneReq>,
) -> Result<Json<Milestone>, AppError> {
    auth::check(&state, &headers).await?;
    validate_milestone(&req.name, &req.date, &req.emoji)?;
    let now = Utc::now().to_rfc3339();
    let milestone: Milestone = sqlx::query_as(
        "INSERT INTO milestones (name, date, emoji, repeat_yearly, created_at)
         VALUES (?, ?, ?, ?, ?)
         RETURNING *",
    )
    .bind(req.name.trim())
    .bind(&req.date)
    .bind(req.emoji.trim())
    .bind(req.repeat_yearly)
    .bind(&now)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(milestone))
}

pub async fn update_milestone(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(req): Json<UpdateMilestoneReq>,
) -> Result<Json<Milestone>, AppError> {
    auth::check(&state, &headers).await?;
    validate_milestone(&req.name, &req.date, &req.emoji)?;
    let milestone: Milestone = sqlx::query_as(
        "UPDATE milestones SET name = ?, date = ?, emoji = ?, repeat_yearly = ?
         WHERE id = ? RETURNING *",
    )
    .bind(req.name.trim())
    .bind(&req.date)
    .bind(req.emoji.trim())
    .bind(req.repeat_yearly)
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(milestone))
}

pub async fn delete_milestone(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth::check(&state, &headers).await?;
    let result = sqlx::query("DELETE FROM milestones WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_image_signatures() {
        assert_eq!(
            detect_image(&[0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10]),
            Some(("jpg", "image/jpeg"))
        );
        assert_eq!(
            detect_image(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0]),
            Some(("png", "image/png"))
        );
        assert_eq!(detect_image(b"not-an-image"), None);
    }

    #[test]
    fn leap_day_anniversary_uses_february_28() {
        let love_start = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
        let today = NaiveDate::from_ymd_opt(2027, 2, 28).unwrap();
        let anniversary = next_anniversary(today, love_start).unwrap();

        assert_eq!(anniversary.name, "3周年");
        assert_eq!(anniversary.date, "2027-02-28");
        assert_eq!(anniversary.days_until, 0);
    }

    #[test]
    fn rejects_unsafe_upload_filenames() {
        assert!(is_safe_filename("abc.jpg"));
        assert!(!is_safe_filename("../abc.jpg"));
        assert!(!is_safe_filename("a/b.jpg"));
        assert!(!is_safe_filename(""));
    }
}
