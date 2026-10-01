use axum::{
    async_trait,
    extract::{FromRequestParts, State},
    http::{header, request::Parts, HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};

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

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginDto>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    if payload.username != state.admin_user || payload.password != state.admin_password {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Usuario o contraseña incorrectos".to_string(),
        ));
    }

    let token = state.issue_token().await;
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
