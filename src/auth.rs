use crate::{error::AppError, AppState};
use axum::http::{header::COOKIE, HeaderMap, HeaderValue};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use uuid::Uuid;

const SESSION_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;
const SESSION_COOKIE: &str = "love_journal_session";
const MAX_LOGIN_FAILURES: u32 = 8;
const LOGIN_BLOCK_SECONDS: u64 = 15 * 60;

#[derive(Debug, Default)]
pub struct LoginLimiter {
    failed_attempts: u32,
    blocked_until: Option<Instant>,
}

impl LoginLimiter {
    pub fn blocked_seconds(&mut self) -> Option<u64> {
        let blocked_until = self.blocked_until?;

        let now = Instant::now();
        if blocked_until > now {
            return Some(blocked_until.duration_since(now).as_secs().max(1));
        }

        self.blocked_until = None;
        self.failed_attempts = 0;
        None
    }

    fn record_failure(&mut self) {
        self.failed_attempts = self.failed_attempts.saturating_add(1);
        if self.failed_attempts >= MAX_LOGIN_FAILURES {
            self.blocked_until = Some(Instant::now() + Duration::from_secs(LOGIN_BLOCK_SECONDS));
            self.failed_attempts = 0;
        }
    }

    fn reset(&mut self) {
        self.failed_attempts = 0;
        self.blocked_until = None;
    }
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    let left = Sha256::digest(left.as_bytes());
    let right = Sha256::digest(right.as_bytes());

    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right) {
        difference |= a ^ b;
    }
    difference == 0
}

fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

async fn delete_expired(state: &AppState) -> Result<(), AppError> {
    sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
        .bind(Utc::now().timestamp())
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn login(state: &AppState, password: &str) -> Result<String, AppError> {
    {
        let mut limiter = state.login_limiter.lock().await;
        if let Some(seconds) = limiter.blocked_seconds() {
            let minutes = seconds.div_ceil(60);
            return Err(AppError::TooManyRequests(format!(
                "登录尝试过多，请在约 {minutes} 分钟后重试"
            )));
        }
    }

    if !constant_time_eq(password, &state.password) {
        state.login_limiter.lock().await.record_failure();
        return Err(AppError::Unauthorized);
    }

    state.login_limiter.lock().await.reset();

    delete_expired(state).await?;

    let token = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    sqlx::query("INSERT INTO sessions (token_hash, expires_at, created_at) VALUES (?, ?, ?)")
        .bind(hash_token(&token))
        .bind(now + SESSION_TTL_SECONDS)
        .bind(now)
        .execute(&state.db)
        .await?;

    Ok(token)
}

pub async fn logout(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    if let Some(token) = extract_token(headers) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(hash_token(&token))
            .execute(&state.db)
            .await?;
    }
    Ok(())
}

pub async fn is_authenticated(state: &AppState, headers: &HeaderMap) -> Result<bool, AppError> {
    let Some(token) = extract_token(headers) else {
        return Ok(false);
    };
    let token_hash = hash_token(&token);
    let expires_at: Option<i64> =
        sqlx::query_scalar("SELECT expires_at FROM sessions WHERE token_hash = ?")
            .bind(&token_hash)
            .fetch_optional(&state.db)
            .await?;

    match expires_at {
        Some(expires_at) if expires_at >= Utc::now().timestamp() => Ok(true),
        Some(_) => {
            sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
                .bind(&token_hash)
                .execute(&state.db)
                .await?;
            Ok(false)
        }
        None => Ok(false),
    }
}

pub async fn check(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    if is_authenticated(state, headers).await? {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

pub fn session_cookie(token: &str, secure: bool) -> HeaderValue {
    let secure_attr = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={SESSION_TTL_SECONDS}{secure_attr}"
    ))
    .expect("session token is always a valid cookie value")
}

pub fn clear_session_cookie(secure: bool) -> HeaderValue {
    let secure_attr = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "love_journal_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0{secure_attr}"
    ))
    .expect("clear session cookie is always valid")
}

pub fn extract_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_string)
        .or_else(|| extract_session_cookie(headers))
}

fn extract_session_cookie(headers: &HeaderMap) -> Option<String> {
    let cookies = headers.get(COOKIE)?.to_str().ok()?;
    for cookie in cookies.split(';') {
        let Some((name, value)) = cookie.trim().split_once('=') else {
            continue;
        };
        if name == SESSION_COOKIE && !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_handles_different_lengths() {
        assert!(constant_time_eq("secret", "secret"));
        assert!(!constant_time_eq("secret", "secre"));
        assert!(!constant_time_eq("secret", "other"));
    }
    #[test]
    fn secure_cookie_flag_is_added_when_requested() {
        assert!(session_cookie("token", true)
            .to_str()
            .unwrap()
            .contains("Secure"));
        assert!(!session_cookie("token", false)
            .to_str()
            .unwrap()
            .contains("Secure"));
    }

    #[test]
    fn login_limiter_blocks_after_repeated_failures() {
        let mut limiter = LoginLimiter::default();
        for _ in 0..MAX_LOGIN_FAILURES {
            limiter.record_failure();
        }
        assert!(limiter.blocked_seconds().is_some());
        limiter.reset();
        assert!(limiter.blocked_seconds().is_none());
    }
}
