mod db;
mod handlers;
mod models;
mod routes;
mod state;

use state::AppState;

#[tokio::main]
async fn main() {
    println!("🍕 Inicializando Pizzería Los Herrera API (Rust + Axum + SQLite)...");

    let db = match db::init_db().await {
        Ok(connection) => {
            println!("✅ Base de datos SQLite inicializada exitosamente");
            connection
        }
        Err(err) => {
            eprintln!("❌ Error crítico al inicializar la base de datos: {}", err);
            std::process::exit(1);
        }
    };

    let state = AppState::new(db);
    let app = routes::create_router(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let host = format!("0.0.0.0:{}", port);
    let listener = match tokio::net::TcpListener::bind(&host).await {
        Ok(l) => l,
        Err(err) => {
            eprintln!("❌ Error al vincular el servidor en {}: {}", host, err);
            std::process::exit(1);
        }
    };

    println!("🚀 Servidor activo ejecutándose en http://{}", host);
    println!("📍 Endpoints disponibles:");
    println!("   - GET  /api/health");
    println!("   - POST /api/auth/login | logout");
    println!("   - GET/POST /api/products");
    println!("   - GET/PUT/DEL /api/products/:id");
    println!("   - GET/POST /api/orders");
    println!("   - GET/DEL /api/orders/:id");
    println!("   - PATCH /api/orders/:id/status");
    println!("⚙️  Admin: http://localhost:3000/admin");

    if let Err(err) = axum::serve(listener, app).await {
        eprintln!("❌ Error en el servidor: {}", err);
    }
}
