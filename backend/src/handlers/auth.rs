use axum::{
    async_trait,
    extract::{FromRequestParts, State},
    http::{header, request::Parts, HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginDto {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
}

/// Extractor: exige `Authorization: Bearer <token>` válido.
pub struct AdminAuth;

#[async_trait]
impl FromRequestParts<AppState> for AdminAuth {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "Se requiere autenticación de administrador".to_string(),
            ))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "Formato de Authorization inválido (use Bearer <token>)".to_string(),
            ))?
            .trim();

        if token.is_empty() || !state.is_token_valid(token).await {
            return Err((
                StatusCode::UNAUTHORIZED,
                "Token inválido o expirado".to_string(),
            ));
        }

        Ok(AdminAuth)
    }
}

/// Clave usada para contabilizar intentos fallidos de login.
/// Se combina IP y usuario para no castigar a usuarios legítimos distintos
/// que compartan IP (por ejemplo, la misma red de la pizzería).
fn throttle_key(headers: &HeaderMap, username: &str) -> String {
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "desconocida".to_string());
    format!("{}|{}", ip, username)
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginDto>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let key = throttle_key(&headers, &payload.username);

    if let Some(secs) = state.login_lockout(&key).await {
        tracing::warn!("Login bloqueado por exceso de intentos: {}", key);
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            format!(
                "Demasiados intentos fallidos. Intenta de nuevo en {} segundos.",
                secs
            ),
        ));
    }

    // Comparación en tiempo constante para no filtrar la contraseña por tiempos.
    let user_ok = bool::from(
        state
            .admin_user
            .as_bytes()
            .ct_eq(payload.username.as_bytes()),
    );
    let pass_ok = bool::from(
        state
            .admin_password
            .as_bytes()
            .ct_eq(payload.password.as_bytes()),
    );

    if !(user_ok && pass_ok) {
        state.record_login_failure(&key).await;
        tracing::warn!(
            "Intento de login fallido para usuario '{}'",
            payload.username
        );
        return Err((
            StatusCode::UNAUTHORIZED,
            "Usuario o contraseña incorrectos".to_string(),
        ));
    }

    state.clear_login_failures(&key).await;
    let token = state.issue_token();
    Ok(Json(LoginResponse { token }))
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> StatusCode {
    if let Some(auth_header) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            state.revoke_token(token.trim()).await;
        }
    }
    StatusCode::NO_CONTENT
}
