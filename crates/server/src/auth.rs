use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{FromRequestParts, State},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;

pub const SESSION_COOKIE: &str = "pjx_session";

#[derive(Debug, Deserialize)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
}

/// Extractor: the authenticated user, or 401.
pub struct CurrentUser {
    pub id: Uuid,
    pub email: String,
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(SESSION_COOKIE)
            .map(|c| c.value().to_string())
            .ok_or_else(unauthorized)?;
        let user_id = projexity_db::sessions::resolve(&state.pool, &token)
            .await
            .map_err(internal_error)?
            .ok_or_else(unauthorized)?;
        let user = projexity_db::users::find_by_id(&state.pool, user_id)
            .await
            .map_err(internal_error)?
            .ok_or_else(unauthorized)?;
        Ok(CurrentUser {
            id: user.id,
            email: user.email,
        })
    }
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({"error": "unauthorized"})),
    )
        .into_response()
}

fn internal_error(err: anyhow::Error) -> Response {
    tracing::error!(?err, "internal error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": "internal error"})),
    )
        .into_response()
}

fn session_cookie(token: String) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::days(
            projexity_db::sessions::SESSION_TTL_DAYS,
        ))
        .build()
}

pub async fn register(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(creds): Json<Credentials>,
) -> Result<(CookieJar, Json<UserResponse>), Response> {
    // Single-operator default: once the first account exists, registration
    // closes unless explicitly opened (PJX_OPEN_REGISTRATION=1). Teams and
    // invites arrive pre-1.0.
    let open = std::env::var("PJX_OPEN_REGISTRATION").is_ok_and(|v| v == "1");
    if !open {
        let existing = projexity_db::users::count(&state.pool)
            .await
            .map_err(internal_error)?;
        if existing > 0 {
            return Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "registration is closed on this instance"
                })),
            )
                .into_response());
        }
    }

    let email = creds.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": "a valid email is required"})),
        )
            .into_response());
    }
    if creds.password.len() < 8 {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": "password must be at least 8 characters"})),
        )
            .into_response());
    }

    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(creds.password.as_bytes(), &salt)
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?
        .to_string();

    let user = projexity_db::users::create(&state.pool, &email, &hash)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                StatusCode::CONFLICT,
                Json(serde_json::json!({"error": "an account with this email already exists"})),
            )
                .into_response()
        })?;

    let token = projexity_db::sessions::create(&state.pool, user.id)
        .await
        .map_err(internal_error)?;
    Ok((
        jar.add(session_cookie(token)),
        Json(UserResponse {
            id: user.id,
            email: user.email,
        }),
    ))
}

pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(creds): Json<Credentials>,
) -> Result<(CookieJar, Json<UserResponse>), Response> {
    let email = creds.email.trim().to_lowercase();
    let invalid = || {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "invalid email or password"})),
        )
            .into_response()
    };

    let user = projexity_db::users::find_by_email(&state.pool, &email)
        .await
        .map_err(internal_error)?
        .ok_or_else(invalid)?;

    let parsed =
        PasswordHash::new(&user.password_hash).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    Argon2::default()
        .verify_password(creds.password.as_bytes(), &parsed)
        .map_err(|_| invalid())?;

    let token = projexity_db::sessions::create(&state.pool, user.id)
        .await
        .map_err(internal_error)?;
    Ok((
        jar.add(session_cookie(token)),
        Json(UserResponse {
            id: user.id,
            email: user.email,
        }),
    ))
}

pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), Response> {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        projexity_db::sessions::delete(&state.pool, cookie.value())
            .await
            .map_err(internal_error)?;
    }
    Ok((
        jar.remove(Cookie::from(SESSION_COOKIE)),
        StatusCode::NO_CONTENT,
    ))
}

pub async fn me(user: CurrentUser) -> Json<UserResponse> {
    Json(UserResponse {
        id: user.id,
        email: user.email,
    })
}
