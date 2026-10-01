use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, FromQueryResult, Statement, TransactionTrait,
};
use uuid::Uuid;
use chrono::Utc;

use crate::handlers::auth::AdminAuth;
use crate::models::order::{
    CreateOrderDto, Order, OrderItem, OrderWithItems, UpdateOrderStatusDto,
};
use crate::models::product::Product;
use crate::state::AppState;

const VALID_STATUSES: &[&str] = &["Pendiente", "En Preparación", "En Camino", "Entregado"];

pub async fn create_order(
    State(state): State<AppState>,
    Json(payload): Json<CreateOrderDto>,
) -> Result<(StatusCode, Json<OrderWithItems>), (StatusCode, String)> {
    if payload.items.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "El pedido debe contener al menos un producto".to_string(),
        ));
    }

    for item_dto in &payload.items {
        if item_dto.quantity <= 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                "La cantidad de cada producto debe ser mayor que 0".to_string(),
            ));
        }
    }

    let db = &state.db;
    let order_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    let notes = payload.notes.unwrap_or_default();

    let mut total_amount = 0.0;
    let mut order_items: Vec<OrderItem> = Vec::new();

    for item_dto in &payload.items {
        let sql_prod = "SELECT id, name, description, price, category, available, created_at FROM products WHERE id = ?";
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
                format!("El producto '{}' no está disponible actualmente", product.name),
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

pub async fn list_orders(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> Result<Json<Vec<OrderWithItems>>, (StatusCode, String)> {
    let db = &state.db;
    let sql_orders = "SELECT id, customer_name, customer_id_number, customer_phone, delivery_address, latitude, longitude, status, total_amount, notes, created_at FROM orders ORDER BY created_at DESC";
    let stmt_orders = Statement::from_string(DatabaseBackend::Sqlite, sql_orders);

    let orders = Order::find_by_statement(stmt_orders)
        .all(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut result = Vec::new();

    for order in orders {
        let sql_items = "SELECT id, order_id, product_id, product_name, quantity, unit_price, subtotal FROM order_items WHERE order_id = ?";
        let stmt_items = Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sql_items,
            vec![order.id.clone().into()],
        );

        let items = OrderItem::find_by_statement(stmt_items)
            .all(db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let maps_url = build_google_maps_url(order.latitude, order.longitude, &order.delivery_address);
        let wa_url = build_whatsapp_url(&order, &items);

        result.push(OrderWithItems {
            order,
            items,
            google_maps_url: Some(maps_url),
            whatsapp_url: Some(wa_url),
        });
    }

    Ok(Json(result))
}

pub async fn get_order(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<OrderWithItems>, (StatusCode, String)> {
    let db = &state.db;
    let sql_order = "SELECT id, customer_name, customer_id_number, customer_phone, delivery_address, latitude, longitude, status, total_amount, notes, created_at FROM orders WHERE id = ?";
    let stmt_order =
        Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql_order, vec![id.clone().into()]);

    let order = Order::find_by_statement(stmt_order)
        .one(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Pedido no encontrado".to_string()))?;

    let sql_items = "SELECT id, order_id, product_id, product_name, quantity, unit_price, subtotal FROM order_items WHERE order_id = ?";
    let stmt_items =
        Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql_items, vec![id.into()]);

    let items = OrderItem::find_by_statement(stmt_items)
        .all(db)
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

pub async fn update_order_status(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateOrderStatusDto>,
) -> Result<Json<Order>, (StatusCode, String)> {
    if !VALID_STATUSES.contains(&payload.status.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Estado inválido. Use uno de: {}",
                VALID_STATUSES.join(", ")
            ),
        ));
    }

    let db = &state.db;
    let sql_update = "UPDATE orders SET status = ? WHERE id = ?";
    let stmt_update = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql_update,
        vec![payload.status.clone().into(), id.clone().into()],
    );

    let res = db
        .execute(stmt_update)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if res.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Pedido no encontrado".to_string()));
    }

    let sql_select = "SELECT id, customer_name, customer_id_number, customer_phone, delivery_address, latitude, longitude, status, total_amount, notes, created_at FROM orders WHERE id = ?";
    let stmt_select =
        Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql_select, vec![id.into()]);

    let updated_order = Order::find_by_statement(stmt_select)
        .one(db)
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

fn build_whatsapp_url(order: &Order, items: &[OrderItem]) -> String {
    let pizzeria_phone = std::env::var("PIZZERIA_WHATSAPP")
        .unwrap_or_else(|_| "5350722776".to_string())
        .replace(['+', ' ', '-'], "");

    let mut msg = format!(
        "PEDIDO - Pizzería Los Herrera\n\n#{}\nCliente: {}\nCI: {}\nDireccion: {}\n\n",
        &order.id[..8],
        order.customer_name,
        order.customer_id_number,
        order.delivery_address
    );

    for item in items {
        msg.push_str(&format!(
            "- {}x {} ${:.2}\n",
            item.quantity, item.product_name, item.subtotal
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
        msg.push_str(&format!("\nNotas: {}", order.notes));
    }

    let encoded_msg = urlencoding::encode(&msg);
    format!("https://wa.me/{}?text={}", pizzeria_phone, encoded_msg)
}
