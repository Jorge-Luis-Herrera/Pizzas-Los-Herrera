# Step by Step - Creación del proyecto

Estas son las notas de la puesta en marcha original, ordenadas.

## Paso 1: Creación en Rust

    cd [Tu_Dirección]
    cargo new [Nombre] --bin
    cd [Nombre]

## Paso 2: Dependencias en Cargo.toml

    axum = { version = "0.7", features = ["multipart"] }
    tokio = { version = "1.0", features = ["full"] }
    serde = { version = "1.0", features = ["derive"] }
    serde_json = "1.0"
    sea-orm = { version = "1.0", features = ["sqlx-sqlite", "runtime-tokio-native-tls"] }
    tower-http = { version = "0.6", features = ["cors", "fs", "trace"] }
    uuid = { version = "1.0", features = ["v4", "serde"] }
    chrono = { version = "0.4", features = ["serde"] }
    urlencoding = "2.1"
    multer = "3"
    tracing = "0.1"
    tracing-subscriber = { version = "0.3", features = ["env-filter"] }
    hmac = "0.12"
    sha2 = "0.10"
    base64 = "0.22"
    subtle = "2.6"

## Paso 3: Estructura del proyecto

    backend/src/
      main.rs            arranque, logging y cierre ordenado
      lib.rs             expone los módulos para los tests
      routes.rs          router, CORS, límite de cuerpo, health check
      state.rs           tokens firmados y rate limit de login
      db.rs              esquema, migración idempotente
      handlers/          auth.rs, products.rs, orders.rs
      models/            product.rs, order.rs
    backend/tests/       tests de integración y de validación
    frontend/            index.html + app.js (cliente)
                         admin.html + admin.js (panel)
    deploy/              systemd, nginx, script de despliegue

## Paso 4: Ejecutar

    cd backend
    cargo run

Abre `http://localhost:3000` para la tienda y `http://localhost:3000/admin` para el panel.

## Paso 5: Tests antes de publicar

    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test

## Notas importantes

- **Nunca** subas contraseñas reales al repositorio. Las credenciales de
  producción viven en secretos de GitHub Actions o en `/etc/pizzeria/pizzeria.env`.
- Los totales de los pedidos se calculan en el servidor a partir del precio de
  la base de datos. El cliente solo envía `product_id` y `quantity`.
- El token de admin va firmado con HMAC y es válido 12 horas; sobrevive a
  reinicios del contenedor.
- `panic = "abort"` **no** debe activarse en release: un panic en un handler
  mataría todo el proceso.
- La app es same-origin, así que `ALLOWED_ORIGINS` se deja vacío salvo que
  necesites llamar a la API desde otro dominio.