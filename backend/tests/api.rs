//! Tests de integración contra el router real.
//!
//! Montan la aplicación con una base de datos SQLite temporal y ejercitan los
//! endpoints por HTTP. Cada test usa su propia base para no interferir.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use pizzeria_api::{db, routes, state::AppState};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU32, Ordering};
use tower::ServiceExt;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Construye el router con una BD temporal y devuelve un login de admin válido.
async fn setup() -> (axum::Router, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let db_path = dir.path().join(format!("test-{}.db", n));

    // `ADMIN_USER` / `ADMIN_PASSWORD` son constantes en todos los tests, así
    // que fijarlos una vez no genera carreras. La ruta de la base se pasa de
    // forma explícita: `DATABASE_URL` es global al proceso y los tests corren
    // en paralelo.
    std::env::set_var("ADMIN_USER", "admin");
    std::env::set_var("ADMIN_PASSWORD", "clave-de-prueba");

    let conn = db::init_db_with_url(&format!("sqlite://{}?mode=rwc", db_path.display()))
        .await
        .expect("init_db");
    let app = routes::create_router(AppState::new(conn));

    let token = login(&app).await;
    (app, token, dir)
}

async fn login(app: &axum::Router) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "username": "admin", "password": "clave-de-prueba" }).to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK, "el login debe funcionar");
    let body = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    serde_json::from_slice::<Value>(&body).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn post_json(
    app: &axum::Router,
    uri: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut b = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {}", t));
    }
    let res = app
        .clone()
        .oneshot(b.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

async fn delete(app: &axum::Router, uri: &str, token: &str) -> StatusCode {
    let req = Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

async fn get_json(app: &axum::Router, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut b = Request::builder().method("GET").uri(uri);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {}", t));
    }
    let res = app
        .clone()
        .oneshot(b.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

async fn create_product(app: &axum::Router, token: &str, price: f64) -> String {
    let (status, body) = post_json(
        app,
        "/api/products",
        Some(token),
        json!({ "name": "Pizza Test", "price": price, "category": "Pizza" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "crear producto debe funcionar: {}",
        body
    );
    body["id"].as_str().unwrap().to_string()
}

/// Regresión del bug que impedía eliminar productos.
/// `DELETE /api/products/:id` devolvía 500 porque el SELECT de una sola
/// columna se deserializaba en el struct `Product` completo.
#[tokio::test]
async fn eliminar_producto_devuelve_204() {
    let (app, token, _dir) = setup().await;
    let id = create_product(&app, &token, 10.0).await;

    assert_eq!(
        delete(&app, &format!("/api/products/{}", id), &token).await,
        StatusCode::NO_CONTENT
    );

    let (status, _) = get_json(&app, &format!("/api/products/{}", id), None).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "el producto debe desaparecer"
    );
}

#[tokio::test]
async fn eliminar_producto_inexistente_devuelve_404() {
    let (app, token, _dir) = setup().await;
    assert_eq!(
        delete(&app, "/api/products/no-existe", &token).await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn eliminar_producto_requiere_autenticacion() {
    let (app, _token, _dir) = setup().await;
    let id = create_product(&app, &_token, 10.0).await;
    assert_eq!(
        delete(&app, &format!("/api/products/{}", id), "").await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn rechaza_precio_negativo_por_api() {
    let (app, token, _dir) = setup().await;
    let (status, _) = post_json(
        &app,
        "/api/products",
        Some(&token),
        json!({ "name": "Precio Mal", "price": -100.0 }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn el_total_del_pedido_usa_el_precio_de_la_base_datos() {
    let (app, token, _dir) = setup().await;
    let id = create_product(&app, &token, 12.5).await;

    let (status, body) = post_json(
        &app,
        "/api/orders",
        None,
        json!({
            "customer_name": "Cliente",
            "delivery_address": "Calle 1",
            "items": [{ "product_id": id, "quantity": 3 }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["total_amount"], 37.5);
}

#[tokio::test]
async fn rechaza_pedido_con_campos_vacios() {
    let (app, token, _dir) = setup().await;
    let id = create_product(&app, &token, 10.0).await;

    let (status, _) = post_json(
        &app,
        "/api/orders",
        None,
        json!({
            "customer_name": "",
            "delivery_address": "",
            "items": [{ "product_id": id, "quantity": 1 }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rechaza_pedido_de_producto_agotado() {
    let (app, token, _dir) = setup().await;
    let id = create_product(&app, &token, 10.0).await;

    // Marcar como no disponible
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/api/products/{}", id))
        .header("authorization", format!("Bearer {}", token))
        .header("content-type", "application/json")
        .body(Body::from(json!({ "available": false }).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::OK
    );

    let (status, _) = post_json(
        &app,
        "/api/orders",
        None,
        json!({
            "customer_name": "Cliente",
            "delivery_address": "Calle 1",
            "items": [{ "product_id": id, "quantity": 1 }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn los_pedidos_exigen_autenticacion() {
    let (app, _token, _dir) = setup().await;
    let (status, _) = get_json(&app, "/api/orders", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn health_check_consulta_la_base_de_datos() {
    let (app, _token, _dir) = setup().await;
    let (status, _) = get_json(&app, "/api/health", None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn cambia_el_estado_del_pedido_y_rechaza_estados_invalidos() {
    let (app, token, _dir) = setup().await;
    let id = create_product(&app, &token, 10.0).await;
    let (_, order) = post_json(
        &app,
        "/api/orders",
        None,
        json!({
            "customer_name": "Cliente",
            "delivery_address": "Calle 1",
            "items": [{ "product_id": id, "quantity": 1 }]
        }),
    )
    .await;
    let oid = order["id"].as_str().unwrap().to_string();

    let set_status = |value: String, app: axum::Router, token: String, oid: String| async move {
        let req = Request::builder()
            .method("PATCH")
            .uri(format!("/api/orders/{}/status", oid))
            .header("authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(json!({ "status": value }).to_string()))
            .unwrap();
        app.oneshot(req).await.unwrap().status()
    };

    let status = set_status(
        "En Preparación".into(),
        app.clone(),
        token.clone(),
        oid.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let status = set_status("Hackeado".into(), app.clone(), token.clone(), oid.clone()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn permite_corregir_la_direccion_de_un_pedido() {
    let (app, token, _dir) = setup().await;
    let id = create_product(&app, &token, 10.0).await;
    let (_, order) = post_json(
        &app,
        "/api/orders",
        None,
        json!({
            "customer_name": "Cliente",
            "delivery_address": "Direccion Equivocada",
            "items": [{ "product_id": id, "quantity": 1 }]
        }),
    )
    .await;
    let oid = order["id"].as_str().unwrap().to_string();

    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/api/orders/{}", oid))
        .header("authorization", format!("Bearer {}", token))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "delivery_address": "Calle 50, Num 12", "customer_phone": "5551234" })
                .to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    let updated: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(updated["delivery_address"], "Calle 50, Num 12");
    assert_eq!(updated["customer_phone"], "5551234");
}

#[tokio::test]
async fn login_incorrecto_es_401() {
    let (app, _token, _dir) = setup().await;
    let (status, _) = post_json(
        &app,
        "/api/auth/login",
        None,
        json!({ "username": "admin", "password": "incorrecta" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_invalida_el_token() {
    let (app, token, _dir) = setup().await;
    let id = create_product(&app, &token, 10.0).await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/logout")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::NO_CONTENT
    );

    // Tras el logout el token ya no debe servir
    assert_eq!(
        delete(&app, &format!("/api/products/{}", id), &token).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn bloquea_el_login_tras_varios_intentos_fallidos() {
    let (app, token, _dir) = setup().await;

    for _ in 0..5 {
        let _ = post_json(
            &app,
            "/api/auth/login",
            None,
            json!({ "username": "admin", "password": "mal" }),
        )
        .await;
    }

    // Ahora un login correcto también debe quedar bloqueado (429).
    let (status, _) = post_json(
        &app,
        "/api/auth/login",
        None,
        json!({ "username": "admin", "password": "clave-de-prueba" }),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    drop(token);
}

#[tokio::test]
async fn listar_pedidos_trae_los_items_sin_consultas_extra() {
    // Verifica que el JOIN devuelve los ítems correctamente asociarlos.
    let (app, token, _dir) = setup().await;
    let a = create_product(&app, &token, 5.0).await;
    let b = create_product(&app, &token, 7.0).await;

    let (_, order) = post_json(
        &app,
        "/api/orders",
        None,
        json!({
            "customer_name": "Cliente",
            "delivery_address": "Calle 1",
            "items": [
                { "product_id": a, "quantity": 1 },
                { "product_id": b, "quantity": 2 }
            ]
        }),
    )
    .await;
    let oid = order["id"].as_str().unwrap().to_string();

    let (status, body) = get_json(&app, &format!("/api/orders/{}", oid), Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn el_mensaje_de_whatsapp_se_acota() {
    let (app, token, _dir) = setup().await;
    let id = create_product(&app, &token, 5.0).await;
    let direccion_larga = "Calle exceedingly larga ".repeat(200);

    let (status, body) = post_json(
        &app,
        "/api/orders",
        None,
        json!({
            "customer_name": "Cliente",
            "delivery_address": direccion_larga,
            "items": [{ "product_id": id, "quantity": 1 }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let wa = body["whatsapp_url"].as_str().unwrap();
    assert!(
        wa.len() < 2500,
        "la URL de WhatsApp debe quedar acotada, iba de {} caracteres",
        wa.len()
    );
}
