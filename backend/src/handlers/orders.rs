use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement, TransactionTrait};
use uuid::Uuid;

use crate::handlers::auth::AdminAuth;
use crate::models::order::{
    CreateOrderDto, Order, OrderItem, OrderWithItems, UpdateOrderDto, UpdateOrderStatusDto,
};
use crate::models::product::Product;
use crate::state::AppState;

const VALID_STATUSES: &[&str] = &["Pendiente", "En Preparación", "En Camino", "Entregado"];

const ORDER_COLS: &str = "id, customer_name, customer_id_number, customer_phone, delivery_address, latitude, longitude, status, total_amount, notes, created_at";

/// Máximo de caracteres por campo en el mensaje de WhatsApp.
/// wa.me no tolera URLs muy largas; los pedidos grandes deben resumirse.
const WA_NAME_MAX: usize = 60;
const WA_ADDRESS_MAX: usize = 160;
const WA_NOTES_MAX: usize = 300;

#[derive(Debug, serde::Deserialize)]
pub struct OrdersQuery {
    /// Filtra por estado. Si se omite, devuelve todos.
    pub status: Option<String>,
    /// Límite de pedidos a devolver (por defecto 200, máximo 1000).
    pub limit: Option<u32>,
}

/// Valida que `status` sea una transición conocida.
fn is_valid_status(status: &str) -> bool {
    VALID_STATUSES.contains(&status)
}

pub async fn create_order(
    State(state): State<AppState>,
    Json(payload): Json<CreateOrderDto>,
) -> Result<(StatusCode, Json<OrderWithItems>), (StatusCode, String)> {
    let payload = match payload.sanitize() {
        Ok(p) => p,
        Err(msg) => return Err((StatusCode::BAD_REQUEST, msg)),
    };

    let db = &state.db;
    let order_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    let notes = payload.notes.clone().unwrap_or_default();

    let mut total_amount = 0.0;
    let mut order_items: Vec<OrderItem> = Vec::new();

    // Los precios SIEMPRE se toman de la base de datos: el cliente nunca
    // puede decidir cuánto paga.
    for item_dto in &payload.items {
        let sql_prod = format!(
            "SELECT {} FROM products WHERE id = ?",
            crate::handlers::products::PRODUCT_COLS
        );
        let stmt_prod = Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sql_prod,
            vec![item_dto.product_id.clone().into()],
        );

        let product = Product::find_by_statement(stmt_prod)
            .one(db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .ok_or((
                StatusCode::BAD_REQUEST,
                format!("Producto con ID {} no existe", item_dto.product_id),
            ))?;

        if !product.available {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "El producto '{}' no está disponible actualmente",
                    product.name
                ),
            ));
        }

        let item_id = Uuid::new_v4().to_string();
        let subtotal = product.price * (item_dto.quantity as f64);
        total_amount += subtotal;

        order_items.push(OrderItem {
            id: item_id,
            order_id: order_id.clone(),
            product_id: product.id,
            product_name: product.name,
            quantity: item_dto.quantity,
            unit_price: product.price,
            subtotal,
        });
    }

    // Redondeo a 2 decimales para que el total mostrado coincida con la suma
    // de los subtotales (los floats acumulan error binario).
    total_amount = (total_amount * 100.0).round() / 100.0;

    let txn = db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let sql_order = "INSERT INTO orders (id, customer_name, customer_id_number, customer_phone, delivery_address, latitude, longitude, status, total_amount, notes, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, 'Pendiente', ?, ?, ?)";
    let stmt_order = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql_order,
        vec![
            order_id.clone().into(),
            payload.customer_name.clone().into(),
            payload.customer_id_number.clone().into(),
            payload.customer_phone.clone().into(),
            payload.delivery_address.clone().into(),
            payload.latitude.into(),
            payload.longitude.into(),
            total_amount.into(),
            notes.clone().into(),
            created_at.clone().into(),
        ],
    );

    txn.execute(stmt_order)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for item in &order_items {
        let sql_item = "INSERT INTO order_items (id, order_id, product_id, product_name, quantity, unit_price, subtotal) VALUES (?, ?, ?, ?, ?, ?, ?)";
        let stmt_item = Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sql_item,
            vec![
                item.id.clone().into(),
                item.order_id.clone().into(),
                item.product_id.clone().into(),
                item.product_name.clone().into(),
                item.quantity.into(),
                item.unit_price.into(),
                item.subtotal.into(),
            ],
        );

        txn.execute(stmt_item)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    txn.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let order = Order {
        id: order_id,
        customer_name: payload.customer_name,
        customer_id_number: payload.customer_id_number,
        customer_phone: payload.customer_phone,
        delivery_address: payload.delivery_address,
        latitude: payload.latitude,
        longitude: payload.longitude,
        status: "Pendiente".to_string(),
        total_amount,
        notes,
        created_at,
    };

    let maps_url = build_google_maps_url(order.latitude, order.longitude, &order.delivery_address);
    let wa_url = build_whatsapp_url(&order, &order_items);

    Ok((
        StatusCode::CREATED,
        Json(OrderWithItems {
            order,
            items: order_items,
            google_maps_url: Some(maps_url),
            whatsapp_url: Some(wa_url),
        }),
    ))
}

/// Carga todos los pedidos con una sola consulta JOIN, evitando el N+1
/// que hacía `list_orders` (una consulta de ítems por cada pedido).
async fn fetch_orders_with_items(
    db: &sea_orm::DatabaseConnection,
    status: Option<&str>,
    limit: u32,
) -> Result<Vec<OrderWithItems>, String> {
    #[derive(Debug, FromQueryResult)]
    struct Row {
        // Columnas del pedido con alias únicos para no colisionar con las de ítems.
        o_id: String,
        o_customer_name: String,
        o_customer_id_number: String,
        o_customer_phone: String,
        o_delivery_address: String,
        o_latitude: Option<f64>,
        o_longitude: Option<f64>,
        o_status: String,
        o_total_amount: f64,
        o_notes: String,
        o_created_at: String,
        i_id: Option<String>,
        i_product_id: Option<String>,
        i_product_name: Option<String>,
        i_quantity: Option<i32>,
        i_unit_price: Option<f64>,
        i_subtotal: Option<f64>,
    }

    let sql = format!(
        "SELECT o.id AS o_id, o.customer_name AS o_customer_name, \
         o.customer_id_number AS o_customer_id_number, o.customer_phone AS o_customer_phone, \
         o.delivery_address AS o_delivery_address, o.latitude AS o_latitude, \
         o.longitude AS o_longitude, o.status AS o_status, o.total_amount AS o_total_amount, \
         o.notes AS o_notes, o.created_at AS o_created_at, \
         i.id AS i_id, i.product_id AS i_product_id, i.product_name AS i_product_name, \
         i.quantity AS i_quantity, i.unit_price AS i_unit_price, i.subtotal AS i_subtotal \
         FROM orders o \
         LEFT JOIN order_items i ON i.order_id = o.id \
         {where_clause} \
         ORDER BY o.created_at DESC, i.rowid ASC \
         LIMIT {limit}",
        where_clause = if status.is_some() {
            "WHERE o.status = ?"
        } else {
            ""
        },
        limit = limit
    );

    let stmt = match status {
        Some(s) => Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql, vec![s.into()]),
        None => Statement::from_string(DatabaseBackend::Sqlite, sql),
    };

    let rows = Row::find_by_statement(stmt)
        .all(db)
        .await
        .map_err(|e| e.to_string())?;

    let mut orders: Vec<OrderWithItems> = Vec::new();
    for row in rows {
        let order = Order {
            id: row.o_id.clone(),
            customer_name: row.o_customer_name,
            customer_id_number: row.o_customer_id_number,
            customer_phone: row.o_customer_phone,
            delivery_address: row.o_delivery_address,
            latitude: row.o_latitude,
            longitude: row.o_longitude,
            status: row.o_status,
            total_amount: row.o_total_amount,
            notes: row.o_notes,
            created_at: row.o_created_at,
        };

        let items = match row.i_id {
            None => Vec::new(),
            Some(item_id) => vec![OrderItem {
                id: item_id,
                order_id: row.o_id,
                product_id: row.i_product_id.unwrap_or_default(),
                product_name: row.i_product_name.unwrap_or_default(),
                quantity: row.i_quantity.unwrap_or(0),
                unit_price: row.i_unit_price.unwrap_or(0.0),
                subtotal: row.i_subtotal.unwrap_or(0.0),
            }],
        };

        let google_maps_url =
            build_google_maps_url(order.latitude, order.longitude, &order.delivery_address);
        let whatsapp_url = build_whatsapp_url(&order, &items);

        orders.push(OrderWithItems {
            order,
            items,
            google_maps_url: Some(google_maps_url),
            whatsapp_url: Some(whatsapp_url),
        });
    }

    Ok(orders)
}

pub async fn list_orders(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(params): Query<OrdersQuery>,
) -> Result<Json<Vec<OrderWithItems>>, (StatusCode, String)> {
    if let Some(status) = params.status.as_deref() {
        if !is_valid_status(status) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Estado inválido. Use uno de: {}", VALID_STATUSES.join(", ")),
            ));
        }
    }

    let limit = params.limit.unwrap_or(200).clamp(1, 1000);

    let orders = fetch_orders_with_items(&state.db, params.status.as_deref(), limit)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(orders))
}

pub async fn get_order(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<OrderWithItems>, (StatusCode, String)> {
    let sql_order = format!("SELECT {} FROM orders WHERE id = ?", ORDER_COLS);
    let stmt_order =
        Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql_order, vec![id.clone().into()]);

    let order = Order::find_by_statement(stmt_order)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Pedido no encontrado".to_string()))?;

    let sql_items = "SELECT id, order_id, product_id, product_name, quantity, unit_price, subtotal FROM order_items WHERE order_id = ?";
    let stmt_items =
        Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql_items, vec![id.into()]);

    let items = OrderItem::find_by_statement(stmt_items)
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let maps_url = build_google_maps_url(order.latitude, order.longitude, &order.delivery_address);
    let wa_url = build_whatsapp_url(&order, &items);

    Ok(Json(OrderWithItems {
        order,
        items,
        google_maps_url: Some(maps_url),
        whatsapp_url: Some(wa_url),
    }))
}

/// Corrige los datos de entrega de un pedido (dirección, teléfono, notas).
pub async fn update_order(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateOrderDto>,
) -> Result<Json<Order>, (StatusCode, String)> {
    let sql_select = format!("SELECT {} FROM orders WHERE id = ?", ORDER_COLS);
    let stmt_select = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql_select,
        vec![id.clone().into()],
    );

    let existing = Order::find_by_statement(stmt_select)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Pedido no encontrado".to_string()))?;

    let clamp =
        |v: String, max: usize| -> String { v.trim().chars().take(max).collect::<String>() };

    let customer_name = payload
        .customer_name
        .map(|v| clamp(v, crate::models::order::MAX_NAME_LEN))
        .unwrap_or(existing.customer_name);
    if customer_name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "El nombre del cliente no puede quedar vacío".to_string(),
        ));
    }

    let delivery_address = payload
        .delivery_address
        .map(|v| clamp(v, crate::models::order::MAX_ADDRESS_LEN))
        .unwrap_or(existing.delivery_address);
    if delivery_address.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "La dirección de entrega no puede quedar vacía".to_string(),
        ));
    }

    let customer_id_number = payload
        .customer_id_number
        .map(|v| clamp(v, crate::models::order::MAX_ID_NUMBER_LEN))
        .unwrap_or(existing.customer_id_number);
    let customer_phone = payload
        .customer_phone
        .map(|v| clamp(v, crate::models::order::MAX_PHONE_LEN))
        .unwrap_or(existing.customer_phone);
    let notes = payload
        .notes
        .map(|v| clamp(v, crate::models::order::MAX_NOTES_LEN))
        .unwrap_or(existing.notes);

    let latitude = payload.latitude.or(existing.latitude);
    let longitude = payload.longitude.or(existing.longitude);
    if let (Some(lat), Some(lng)) = (latitude, longitude) {
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lng) {
            return Err((
                StatusCode::BAD_REQUEST,
                "Las coordenadas GPS no son válidas".to_string(),
            ));
        }
    }

    let sql_update = "UPDATE orders SET customer_name = ?, customer_id_number = ?, customer_phone = ?, delivery_address = ?, latitude = ?, longitude = ?, notes = ? WHERE id = ?";
    let stmt_update = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql_update,
        vec![
            customer_name.clone().into(),
            customer_id_number.clone().into(),
            customer_phone.clone().into(),
            delivery_address.clone().into(),
            latitude.into(),
            longitude.into(),
            notes.clone().into(),
            id.clone().into(),
        ],
    );

    state
        .db
        .execute(stmt_update)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(Order {
        id,
        customer_name,
        customer_id_number,
        customer_phone,
        delivery_address,
        latitude,
        longitude,
        status: existing.status,
        total_amount: existing.total_amount,
        notes,
        created_at: existing.created_at,
    }))
}

pub async fn update_order_status(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateOrderStatusDto>,
) -> Result<Json<Order>, (StatusCode, String)> {
    if !is_valid_status(&payload.status) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Estado inválido. Use uno de: {}", VALID_STATUSES.join(", ")),
        ));
    }

    let sql_update = "UPDATE orders SET status = ? WHERE id = ?";
    let stmt_update = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql_update,
        vec![payload.status.clone().into(), id.clone().into()],
    );

    let res = state
        .db
        .execute(stmt_update)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if res.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Pedido no encontrado".to_string()));
    }

    let sql_select = format!("SELECT {} FROM orders WHERE id = ?", ORDER_COLS);
    let stmt_select =
        Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql_select, vec![id.into()]);

    let updated_order = Order::find_by_statement(stmt_select)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Pedido no encontrado".to_string()))?;

    Ok(Json(updated_order))
}

pub async fn delete_order(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = &state.db;

    let txn = db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let sql_items = "DELETE FROM order_items WHERE order_id = ?";
    let stmt_items =
        Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql_items, vec![id.clone().into()]);
    txn.execute(stmt_items)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let sql_order = "DELETE FROM orders WHERE id = ?";
    let stmt_order =
        Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql_order, vec![id.into()]);
    let res = txn
        .execute(stmt_order)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    txn.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if res.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Pedido no encontrado".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

fn build_google_maps_url(lat: Option<f64>, lng: Option<f64>, address: &str) -> String {
    match (lat, lng) {
        (Some(latitude), Some(longitude)) => {
            format!("https://www.google.com/maps?q={},{}", latitude, longitude)
        }
        _ => {
            let encoded_address = urlencoding::encode(address);
            format!(
                "https://www.google.com/maps/search/?api=1&query={}",
                encoded_address
            )
        }
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(3)).collect();
        out.push_str("...");
        out
    }
}

fn build_whatsapp_url(order: &Order, items: &[OrderItem]) -> String {
    let pizzeria_phone = std::env::var("PIZZERIA_WHATSAPP")
        .unwrap_or_else(|_| "5350722776".to_string())
        .replace(['+', ' ', '-'], "");

    // Los UUID siempre tienen 36 caracteres, pero se evita el panic por
    // slicing si algún día el id cambia de formato.
    let short_id: String = order.id.chars().take(8).collect();

    let mut msg = format!(
        "PEDIDO - Pizzería Los Herrera\n\n#{}\nCliente: {}\nCI: {}\nDireccion: {}\n\n",
        short_id,
        truncate_chars(&order.customer_name, WA_NAME_MAX),
        order.customer_id_number,
        truncate_chars(&order.delivery_address, WA_ADDRESS_MAX),
    );

    if !order.customer_phone.is_empty() {
        msg.push_str(&format!("Tel: {}\n", order.customer_phone));
    }

    for item in items {
        msg.push_str(&format!(
            "- {}x {} ${:.2}\n",
            item.quantity,
            truncate_chars(&item.product_name, 60),
            item.subtotal
        ));
    }

    msg.push_str(&format!("\nTotal: ${:.2}", order.total_amount));

    if let (Some(lat), Some(lng)) = (order.latitude, order.longitude) {
        msg.push_str(&format!(
            "\n\nUbicacion: https://www.google.com/maps?q={},{}",
            lat, lng
        ));
    }

    if !order.notes.is_empty() {
        msg.push_str(&format!(
            "\nNotas: {}",
            truncate_chars(&order.notes, WA_NOTES_MAX)
        ));
    }

    // wa.me no acepta URLs muy largas: si el mensaje se pasa de largo se
    // recorta la parte menos útil (notas y ubicación) en vez de perder el envío.
    let encoded = urlencoding::encode(&msg);
    const WA_URL_MAX: usize = 1800;
    if encoded.len() <= WA_URL_MAX {
        return format!("https://wa.me/{}?text={}", pizzeria_phone, encoded);
    }

    let mut short = format!(
        "PEDIDO #{}\nCliente: {}\nDireccion: {}\nTotal: ${:.2}\nVer detalle completo en la web.",
        short_id,
        truncate_chars(&order.customer_name, WA_NAME_MAX),
        truncate_chars(&order.delivery_address, WA_ADDRESS_MAX),
        order.total_amount,
    );
    for item in items.iter().take(10) {
        short.push_str(&format!(
            "\n- {}x {} ${:.2}",
            item.quantity,
            truncate_chars(&item.product_name, 40),
            item.subtotal
        ));
    }
    let encoded_short = urlencoding::encode(&short);
    format!("https://wa.me/{}?text={}", pizzeria_phone, encoded_short)
}
