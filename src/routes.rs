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
pub const MAX_VOICE_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_VOICE_BODY_BYTES: usize = MAX_VOICE_BYTES + 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct EntriesQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub q: Option<String>,
    pub limit: Option<i64>,
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

fn parse_duration(raw: &str) -> Result<f64, AppError> {
    let duration = raw
        .trim()
        .parse::<f64>()
        .map_err(|_| AppError::BadRequest("语音时长格式不正确".into()))?;
    if !duration.is_finite() || !(0.0..=86_400.0).contains(&duration) {
        return Err(AppError::BadRequest("语音时长应在 0 到 24 小时之间".into()));
    }
    Ok(duration)
}

fn validate_entry(req: &CreateEntryReq) -> Result<(), AppError> {
    parse_entry_date(&req.date).map_err(AppError::BadRequest)?;
    ensure_text("内容", req.note.trim(), 1, 10_000)?;
    ensure_text("作者", req.author.trim(), 1, 20)?;
    Ok(())
}

fn validate_update(req: &UpdateEntryReq) -> Result<(), AppError> {
    parse_entry_date(&req.date).map_err(AppError::BadRequest)?;
    ensure_text("内容", req.note.trim(), 1, 10_000)?;
    ensure_text("作者", req.author.trim(), 1, 20)?;
    Ok(())
}
fn validate_milestone(
    name: &str,
    date: &str,
    emoji: &str,
    repeat_yearly: i64,
) -> Result<(), AppError> {
    ensure_text("名称", name.trim(), 1, 40)?;
    parse_entry_date(date).map_err(AppError::BadRequest)?;
    ensure_text("图标", emoji.trim(), 0, 16)?;
    if !matches!(repeat_yearly, 0 | 1) {
        return Err(AppError::BadRequest("每年重复参数只能为 0 或 1".into()));
    }
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

    if query.limit.is_some_and(|limit| !(1..=500).contains(&limit)) {
        return Err(AppError::BadRequest(
            "单次读取数量应在 1 到 500 之间".into(),
        ));
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
fn detect_audio(data: &[u8]) -> Option<(&'static str, &'static str)> {
    if data.starts_with(b"OggS") {
        return Some(("ogg", "audio/ogg"));
    }
    if data.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        return Some(("webm", "audio/webm"));
    }
    if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WAVE" {
        return Some(("wav", "audio/wav"));
    }
    if data.len() >= 12 && &data[4..8] == b"ftyp" {
        return Some(("m4a", "audio/mp4"));
    }
    if data.starts_with(b"ID3") || data.first().is_some_and(|byte| byte & 0xe0 == 0xe0) {
        return Some(("mp3", "audio/mpeg"));
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
pub async fn session_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let authenticated = auth::is_authenticated(&state, &headers).await?;
    Ok(Json(serde_json::json!({ "authenticated": authenticated })))
}
pub async fn health(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.db)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn api_not_found() -> AppError {
    AppError::NotFound
}

pub async fn stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<StatsResp>, AppError> {
    auth::check(&state, &headers).await?;

    let today = chrono::Local::now().date_naive();
    let days_together = (today - state.love_start).num_days().max(0);

    let (entry_count, photo_count, comment_count): (i64, i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM entries),
            (SELECT COUNT(*) FROM photos),
            (SELECT COUNT(*) FROM comments)",
    )
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
    if query.limit.is_some() {
        sql.push_str(" LIMIT ?");
    }

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
    if let Some(limit) = query.limit {
        db_query = db_query.bind(limit);
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

    let voice_notes = sqlx::query_as::<_, VoiceNote>(
        "SELECT * FROM voice_notes WHERE entry_id = ? ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(EntryDetail {
        entry,
        photos,
        comments,
        voice_notes,
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
    .bind(req.note.trim())
    .bind(req.author.trim())
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
    .bind(req.note.trim())
    .bind(req.author.trim())
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

    let voice_notes =
        sqlx::query_as::<_, VoiceNote>("SELECT * FROM voice_notes WHERE entry_id = ?")
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

    let mut paths: Vec<PathBuf> = photos
        .iter()
        .map(|photo| state.upload_dir.join(&photo.filename))
        .collect();
    paths.extend(
        voice_notes
            .iter()
            .map(|voice| state.upload_dir.join(&voice.filename)),
    );
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

    if let Err(error) = transaction.commit().await {
        remove_files(&written_paths).await;
        return Err(error.into());
    }

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
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
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
    ensure_text("留言", req.content.trim(), 1, 500)?;
    ensure_text("名字", req.author.trim(), 1, 20)?;
    ensure_text("贴纸", req.sticker.trim(), 0, 32)?;

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
    .bind(req.author.trim())
    .bind(req.content.trim())
    .bind(req.sticker.trim())
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
    validate_milestone(&req.name, &req.date, &req.emoji, req.repeat_yearly)?;
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
    validate_milestone(&req.name, &req.date, &req.emoji, req.repeat_yearly)?;
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
pub async fn review(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ReviewResp>, AppError> {
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
    let voice_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM voice_notes")
        .fetch_one(&state.db)
        .await?;
    let total_words: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(LENGTH(note)), 0) FROM entries")
        .fetch_one(&state.db)
        .await?;
    let first_entry = sqlx::query_as::<_, Entry>("SELECT * FROM entries ORDER BY date, id LIMIT 1")
        .fetch_optional(&state.db)
        .await?;
    let latest_entry =
        sqlx::query_as::<_, Entry>("SELECT * FROM entries ORDER BY date DESC, id DESC LIMIT 1")
            .fetch_optional(&state.db)
            .await?;
    let monthly = sqlx::query_as::<_, MonthlyReview>(
        "SELECT substr(date, 1, 7) AS month, COUNT(*) AS count FROM entries GROUP BY month ORDER BY month",
    )
    .fetch_all(&state.db)
    .await?;
    let busiest_month = monthly
        .iter()
        .max_by_key(|item| item.count)
        .map(|item| item.month.clone());
    let recent_photos =
        sqlx::query_as::<_, Photo>("SELECT * FROM photos ORDER BY created_at DESC LIMIT 8")
            .fetch_all(&state.db)
            .await?;
    let today_text = today.format("%Y-%m-%d").to_string();
    let letter_rows = sqlx::query_as::<_, LetterRow>(
        "SELECT * FROM letters WHERE open_at <= ? ORDER BY open_at, id",
    )
    .bind(&today_text)
    .fetch_all(&state.db)
    .await?;
    let letter_voice_rows = sqlx::query_as::<_, LetterVoiceNote>(
        "SELECT * FROM letter_voice_notes ORDER BY letter_id, created_at",
    )
    .fetch_all(&state.db)
    .await?;
    let mut letter_voices: HashMap<i64, Vec<LetterVoiceNote>> = HashMap::new();
    for voice in letter_voice_rows {
        letter_voices
            .entry(voice.letter_id)
            .or_default()
            .push(voice);
    }
    let unlocked_letters = letter_rows
        .into_iter()
        .map(|row| {
            let voices = letter_voices.remove(&row.id).unwrap_or_default();
            LetterResp {
                id: row.id,
                title: row.title,
                author: row.author,
                open_at: row.open_at,
                created_at: row.created_at,
                opened_at: row.opened_at,
                unlocked: true,
                days_until: 0,
                content: Some(row.content),
                voice_count: voices.len() as i64,
                voice_notes: voices,
            }
        })
        .collect();

    Ok(Json(ReviewResp {
        days_together,
        entry_count,
        photo_count,
        comment_count,
        voice_count,
        total_words,
        first_entry,
        latest_entry,
        busiest_month,
        monthly,
        recent_photos,
        unlocked_letters,
    }))
}

pub async fn list_letters(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<LetterResp>>, AppError> {
    auth::check(&state, &headers).await?;
    let rows = sqlx::query_as::<_, LetterRow>("SELECT * FROM letters ORDER BY open_at, id")
        .fetch_all(&state.db)
        .await?;
    let voice_rows = sqlx::query_as::<_, LetterVoiceNote>(
        "SELECT * FROM letter_voice_notes ORDER BY letter_id, created_at",
    )
    .fetch_all(&state.db)
    .await?;
    let mut voices_by_letter: HashMap<i64, Vec<LetterVoiceNote>> = HashMap::new();
    for voice in voice_rows {
        voices_by_letter
            .entry(voice.letter_id)
            .or_default()
            .push(voice);
    }
    let today = chrono::Local::now().date_naive();
    let letters = rows
        .into_iter()
        .map(|row| {
            let open_date = parse_entry_date(&row.open_at).ok();
            let unlocked = open_date.map(|date| date <= today).unwrap_or(false);
            let days_until = open_date
                .map(|date| (date - today).num_days().max(0))
                .unwrap_or(0);
            let voices = voices_by_letter.remove(&row.id).unwrap_or_default();
            let voice_count = voices.len() as i64;
            LetterResp {
                id: row.id,
                title: row.title,
                author: row.author,
                open_at: row.open_at,
                created_at: row.created_at,
                opened_at: row.opened_at,
                unlocked,
                days_until,
                content: if unlocked { Some(row.content) } else { None },
                voice_count,
                voice_notes: if unlocked { voices } else { Vec::new() },
            }
        })
        .collect();
    Ok(Json(letters))
}

pub async fn create_letter(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateLetterReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth::check(&state, &headers).await?;
    ensure_text("标题", req.title.trim(), 1, 80)?;
    ensure_text("内容", req.content.trim(), 1, 5000)?;
    ensure_text("署名", req.author.trim(), 1, 20)?;
    parse_entry_date(&req.open_at).map_err(AppError::BadRequest)?;
    let now = Utc::now().to_rfc3339();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO letters (title, content, author, open_at, created_at) VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(req.title.trim())
    .bind(req.content.trim())
    .bind(req.author.trim())
    .bind(&req.open_at)
    .bind(&now)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(serde_json::json!({ "id": id })))
}

pub async fn delete_letter(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth::check(&state, &headers).await?;
    let voices = sqlx::query_as::<_, LetterVoiceNote>(
        "SELECT * FROM letter_voice_notes WHERE letter_id = ?",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let result = sqlx::query("DELETE FROM letters WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    let paths: Vec<PathBuf> = voices
        .iter()
        .map(|voice| state.upload_dir.join(&voice.filename))
        .collect();
    remove_files(&paths).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn upload_voice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(entry_id): Path<i64>,
    mut multipart: Multipart,
) -> Result<Json<VoiceNote>, AppError> {
    auth::check(&state, &headers).await?;
    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM entries WHERE id = ?")
        .bind(entry_id)
        .fetch_optional(&state.db)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    let mut duration = 0.0;
    let mut pending: Option<(Vec<u8>, String, String, &'static str)> = None;
    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap_or("").to_string();
        if name == "duration" {
            duration = parse_duration(&field.text().await?)?;
            continue;
        }
        if name != "audio" {
            continue;
        }
        let original_name = field
            .file_name()
            .unwrap_or("voice-note")
            .chars()
            .take(255)
            .collect();
        let data = field.bytes().await?.to_vec();
        if data.is_empty() || data.len() > MAX_VOICE_BYTES {
            return Err(AppError::BadRequest("语音文件不能超过 20 MB".into()));
        }
        let Some((extension, mime)) = detect_audio(&data) else {
            return Err(AppError::BadRequest("不支持这种语音格式".into()));
        };
        pending = Some((data, mime.to_string(), original_name, extension));
    }
    let Some((data, mime, original_name, extension)) = pending else {
        return Err(AppError::BadRequest("请选择要上传的语音".into()));
    };
    let ext = extension;
    let id = Uuid::new_v4().to_string();
    let filename = format!("{id}.{ext}");
    let path = state.upload_dir.join(&filename);
    if let Err(error) = tokio::fs::write(&path, &data).await {
        tracing::error!("failed to write voice note {:?}: {error}", path);
        return Err(AppError::Internal("语音写入失败".into()));
    }
    let now = Utc::now().to_rfc3339();
    let note: VoiceNote = sqlx::query_as(
        "INSERT INTO voice_notes (id, entry_id, filename, original_name, mime, duration_seconds, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(&id)
    .bind(entry_id)
    .bind(&filename)
    .bind(&original_name)
    .bind(&mime)
    .bind(duration)
    .bind(&now)
    .fetch_one(&state.db)
    .await
    .map_err(|error| {
        let _ = std::fs::remove_file(&path);
        AppError::from(error)
    })?;
    Ok(Json(note))
}

pub async fn serve_voice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(filename): Path<String>,
) -> Result<Response, AppError> {
    auth::check(&state, &headers).await?;
    if !is_safe_filename(&filename) {
        return Err(AppError::BadRequest("无效的语音文件名".into()));
    }
    let note: VoiceNote = sqlx::query_as("SELECT * FROM voice_notes WHERE filename = ?")
        .bind(&filename)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let path = state.upload_dir.join(&note.filename);
    let data = tokio::fs::read(&path)
        .await
        .map_err(|_| AppError::NotFound)?;
    let mime = HeaderValue::from_str(&note.mime)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    let mut response = Response::new(Body::from(Bytes::from(data)));
    response.headers_mut().insert(CONTENT_TYPE, mime);
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    Ok(response)
}

pub async fn delete_voice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth::check(&state, &headers).await?;
    let note: VoiceNote = sqlx::query_as("SELECT * FROM voice_notes WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    sqlx::query("DELETE FROM voice_notes WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;
    let _ = tokio::fs::remove_file(state.upload_dir.join(&note.filename)).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}
pub async fn upload_letter_voice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(letter_id): Path<i64>,
    mut multipart: Multipart,
) -> Result<Json<LetterVoiceNote>, AppError> {
    auth::check(&state, &headers).await?;
    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM letters WHERE id = ?")
        .bind(letter_id)
        .fetch_optional(&state.db)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    let mut duration = 0.0;
    let mut pending: Option<(Vec<u8>, String, String, &'static str)> = None;
    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap_or("").to_string();
        if name == "duration" {
            duration = parse_duration(&field.text().await?)?;
            continue;
        }
        if name != "audio" {
            continue;
        }
        let original_name = field
            .file_name()
            .unwrap_or("letter-voice")
            .chars()
            .take(255)
            .collect();
        let data = field.bytes().await?.to_vec();
        if data.is_empty() || data.len() > MAX_VOICE_BYTES {
            return Err(AppError::BadRequest("语音文件不能超过 20 MB".into()));
        }
        let Some((extension, mime)) = detect_audio(&data) else {
            return Err(AppError::BadRequest("不支持这种语音格式".into()));
        };
        pending = Some((data, mime.to_string(), original_name, extension));
    }
    let Some((data, mime, original_name, extension)) = pending else {
        return Err(AppError::BadRequest("请选择要上传的语音".into()));
    };
    let ext = extension;
    let id = Uuid::new_v4().to_string();
    let filename = format!("{id}.{ext}");
    let path = state.upload_dir.join(&filename);
    if let Err(error) = tokio::fs::write(&path, &data).await {
        tracing::error!("failed to write letter voice note {:?}: {error}", path);
        return Err(AppError::Internal("语音写入失败".into()));
    }
    let now = Utc::now().to_rfc3339();
    let note: LetterVoiceNote = sqlx::query_as(
        "INSERT INTO letter_voice_notes (id, letter_id, filename, original_name, mime, duration_seconds, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(&id)
    .bind(letter_id)
    .bind(&filename)
    .bind(&original_name)
    .bind(&mime)
    .bind(duration)
    .bind(&now)
    .fetch_one(&state.db)
    .await
    .map_err(|error| {
        let _ = std::fs::remove_file(&path);
        AppError::from(error)
    })?;
    Ok(Json(note))
}

pub async fn serve_letter_voice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(filename): Path<String>,
) -> Result<Response, AppError> {
    auth::check(&state, &headers).await?;
    if !is_safe_filename(&filename) {
        return Err(AppError::BadRequest("无效的语音文件名".into()));
    }
    let note: LetterVoiceNote =
        sqlx::query_as("SELECT * FROM letter_voice_notes WHERE filename = ?")
            .bind(&filename)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;
    let open_at: String = sqlx::query_scalar("SELECT open_at FROM letters WHERE id = ?")
        .bind(note.letter_id)
        .fetch_one(&state.db)
        .await?;
    let open_date = parse_entry_date(&open_at).map_err(AppError::BadRequest)?;
    if open_date > chrono::Local::now().date_naive() {
        return Err(AppError::NotFound);
    }
    let path = state.upload_dir.join(&note.filename);
    let data = tokio::fs::read(&path)
        .await
        .map_err(|_| AppError::NotFound)?;
    let mime = HeaderValue::from_str(&note.mime)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    let mut response = Response::new(Body::from(Bytes::from(data)));
    response.headers_mut().insert(CONTENT_TYPE, mime);
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    Ok(response)
}

pub async fn delete_letter_voice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth::check(&state, &headers).await?;
    let note: LetterVoiceNote = sqlx::query_as("SELECT * FROM letter_voice_notes WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    sqlx::query("DELETE FROM letter_voice_notes WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;
    let _ = tokio::fs::remove_file(state.upload_dir.join(&note.filename)).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}
pub async fn letter_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<LetterNotification>>, AppError> {
    auth::check(&state, &headers).await?;
    let today = chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let notifications = sqlx::query_as::<_, LetterNotification>(
        "SELECT id, title, open_at FROM letters WHERE open_at <= ? AND opened_at IS NULL ORDER BY open_at, id",
    )
    .bind(today)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(notifications))
}

pub async fn open_letter(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth::check(&state, &headers).await?;
    let today = chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let result = sqlx::query(
        "UPDATE letters SET opened_at = ? WHERE id = ? AND open_at <= ? AND opened_at IS NULL",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(id)
    .bind(today)
    .execute(&state.db)
    .await?;
    if result.rows_affected() == 0 {
        let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM letters WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
        if exists.is_none() {
            return Err(AppError::NotFound);
        }
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
    fn detects_supported_audio_signatures() {
        assert_eq!(detect_audio(b"OggS\0\0"), Some(("ogg", "audio/ogg")));
        assert_eq!(
            detect_audio(&[0x1a, 0x45, 0xdf, 0xa3, 0x01, 0x00]),
            Some(("webm", "audio/webm"))
        );
        assert_eq!(
            detect_audio(b"RIFF\0\0\0\0WAVEfmt "),
            Some(("wav", "audio/wav"))
        );
        assert_eq!(detect_audio(b"not-audio"), None);
    }

    #[test]
    fn validates_voice_duration() {
        assert_eq!(parse_duration("12.5").unwrap(), 12.5);
        assert!(parse_duration("-1").is_err());
        assert!(parse_duration("NaN").is_err());
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
