use axum::{
    extract::{DefaultBodyLimit, State},
    http::{HeaderValue, Method, StatusCode},
    routing::{get, patch, post},
    Router,
};
use std::path::Path;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::handlers::{auth, orders, products};
use crate::state::AppState;

/// Tope del cuerpo de la petición. El upload de imágenes admite hasta 5 MB
/// (más el multipart overhead), de ahí que se supere el límite de 2 MB por defecto.
const MAX_BODY_BYTES: usize = 6 * 1024 * 1024;

/// Orígenes permitidos. La app es same-origin, así que por defecto no se
/// permite ningún origen cruzado; se amplía con `ALLOWED_ORIGINS`
/// (lista separada por comas). Una lista vacía bloquea todos los orígenes.
fn cors_layer(origins: Vec<String>) -> tower_http::cors::CorsLayer {
    use tower_http::cors::{AllowOrigin, CorsLayer};

    let allow_origin = AllowOrigin::list(
        origins
            .into_iter()
            .filter_map(|o| HeaderValue::from_str(&o).ok())
            .collect::<Vec<_>>(),
    );

    CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ])
}

pub fn create_router(state: AppState) -> Router {
    let origins: Vec<String> = std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let frontend_path = if Path::new("frontend").exists() {
        "frontend"
    } else if Path::new("../frontend").exists() {
        "../frontend"
    } else {
        "frontend"
    };

    let uploads_dir = std::env::var("UPLOADS_DIR").unwrap_or_else(|_| "uploads".to_string());
    let admin_file_path = format!("{}/admin.html", frontend_path);

    Router::new()
        .route("/api/health", get(health_check))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route_service("/admin", ServeFile::new(admin_file_path))
        .route(
            "/api/products",
            get(products::list_products).post(products::create_product),
        )
        .route(
            "/api/products/:id",
            get(products::get_product)
                .put(products::update_product)
                .delete(products::delete_product),
        )
        .route("/api/products/:id/image", post(products::upload_image))
        .route(
            "/api/orders",
            get(orders::list_orders).post(orders::create_order),
        )
        .route(
            "/api/orders/:id",
            get(orders::get_order)
                .patch(orders::update_order)
                .delete(orders::delete_order),
        )
        .route("/api/orders/:id/status", patch(orders::update_order_status))
        .nest_service("/uploads", ServeDir::new(uploads_dir))
        .fallback_service(ServeDir::new(frontend_path))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer(origins))
        .with_state(state)
}

/// Health check que realmente consulta la base de datos.
/// Devolver una constante haría que el orquestador uphold la app mientras
/// SQLite está corrupto o el almacenamiento no está montado.
async fn health_check(State(state): State<AppState>) -> Result<&'static str, StatusCode> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    state
        .db
        .execute(Statement::from_string(DatabaseBackend::Sqlite, "SELECT 1"))
        .await
        .map_err(|err| {
            tracing::error!("Health check falló: {}", err);
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    Ok("🍕 Pizzería Los Herrera API - Backend Rust (Axum + SQLite) activo!")
}
