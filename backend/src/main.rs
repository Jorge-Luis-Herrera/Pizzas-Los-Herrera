use pizzeria_api::{db, routes, state};

use state::AppState;

/// Inicializa el logging estructurado antes de nada.
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,tower_http=info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}

#[tokio::main]
async fn main() {
    init_tracing();
    tracing::info!("🍕 Iniciando Pizzería Los Herrera API (Rust + Axum + SQLite)...");

    let db = match db::init_db().await {
        Ok(connection) => {
            tracing::info!("✅ Base de datos SQLite inicializada exitosamente");
            connection
        }
        Err(err) => {
            tracing::error!("❌ Error crítico al inicializar la base de datos: {}", err);
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
            tracing::error!("❌ Error al vincular el servidor en {}: {}", host, err);
            std::process::exit(1);
        }
    };

    tracing::info!("🚀 Servidor activo ejecutándose en http://{}", host);
    tracing::info!("   - GET  /api/health");
    tracing::info!("   - POST /api/auth/login | logout");
    tracing::info!("   - GET/POST /api/products");
    tracing::info!("   - GET/PUT/DEL /api/products/:id");
    tracing::info!("   - POST /api/products/:id/image");
    tracing::info!("   - GET/POST /api/orders");
    tracing::info!("   - GET/PATCH/DEL /api/orders/:id");
    tracing::info!("   - PATCH /api/orders/:id/status");
    tracing::info!("⚙️  Admin: http://localhost:{}/admin", port);

    // Cierre ordenado: al desplegar, Azure Container Apps envía SIGTERM.
    // Sin esto se cortarían peticiones en curso.
    let shutdown = async {
        let ctrl_c = async {
            let _ = tokio::signal::ctrl_c().await;
        };

        #[cfg(unix)]
        let terminate = async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut sig) => {
                    sig.recv().await;
                }
                Err(err) => {
                    tracing::warn!("No se pudo instalar SIGTERM: {}", err);
                    std::future::pending::<()>().await;
                }
            }
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
        tracing::info!("Señal de cierre recibida, terminando...");
    };

    if let Err(err) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
    {
        tracing::error!("❌ Error en el servidor: {}", err);
    }
}
