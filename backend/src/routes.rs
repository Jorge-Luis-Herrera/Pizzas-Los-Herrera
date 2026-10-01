use axum::{
    routing::{get, patch, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use std::path::Path;

use crate::handlers::{auth, orders, products};
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

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
            get(orders::get_order).delete(orders::delete_order),
        )
        .route("/api/orders/:id/status", patch(orders::update_order_status))
        .nest_service("/uploads", ServeDir::new(uploads_dir))
        .fallback_service(ServeDir::new(frontend_path))
        .layer(cors)
        .with_state(state)
}

async fn health_check() -> &'static str {
    "🍕 Pizzería Los Herrera API - Backend Rust (Axum + SQLite) activo!"
}
