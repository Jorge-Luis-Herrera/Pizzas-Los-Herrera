//! Tests de sanitización de los datos de un pedido.

use pizzeria_api::models::order::{CreateOrderDto, MAX_ITEM_QUANTITY};
use serde_json::json;

fn dto(items: serde_json::Value) -> CreateOrderDto {
    let payload = json!({
        "customer_name": "Cliente",
        "customer_id_number": "90010112345",
        "customer_phone": "55555555",
        "delivery_address": "Calle 1",
        "items": items,
    });
    serde_json::from_value(payload).expect("el DTO base debe ser válido")
}

#[test]
fn acepta_pedido_valido() {
    let ok = dto(json!([{ "product_id": "abc", "quantity": 2 }]))
        .sanitize()
        .is_ok();
    assert!(ok);
}

#[test]
fn rechaza_nombre_vacio() {
    let d = dto(json!([{ "product_id": "abc", "quantity": 1 }]));
    let d = CreateOrderDto {
        customer_name: "   ".to_string(),
        ..d
    };
    assert!(d.sanitize().is_err());
}

#[test]
fn rechaza_direccion_vacia() {
    let d = dto(json!([{ "product_id": "abc", "quantity": 1 }]));
    let d = CreateOrderDto {
        delivery_address: "".to_string(),
        ..d
    };
    assert!(d.sanitize().is_err());
}

#[test]
fn rechaza_pedido_sin_items() {
    assert!(dto(json!([])).sanitize().is_err());
}

#[test]
fn rechaza_cantidad_cero_o_negativa() {
    assert!(dto(json!([{ "product_id": "abc", "quantity": 0 }]))
        .sanitize()
        .is_err());
    assert!(dto(json!([{ "product_id": "abc", "quantity": -3 }]))
        .sanitize()
        .is_err());
}

#[test]
fn rechaza_cantidad_absurda() {
    // Regresión: se aceptaban 99.999.999 unidades.
    let mut items = Vec::new();
    items.push(json!({ "product_id": "abc", "quantity": 99_999_999i64 }));
    assert!(dto(json!(items)).sanitize().is_err());
}

#[test]
fn acepta_cantidad_en_el_limite() {
    assert!(
        dto(json!([{ "product_id": "abc", "quantity": MAX_ITEM_QUANTITY }]))
            .sanitize()
            .is_ok()
    );
}

#[test]
fn consolida_lineas_duplicadas_del_mismo_producto() {
    let items = json!([
        { "product_id": "abc", "quantity": 10 },
        { "product_id": "abc", "quantity": 15 },
    ]);
    let d = dto(items).sanitize().expect("debe consolidar");
    assert_eq!(d.items.len(), 1);
    assert_eq!(d.items[0].quantity, 25);
}

#[test]
fn rechaza_coordenadas_fuera_de_rango() {
    let d = dto(json!([{ "product_id": "abc", "quantity": 1 }]));
    let d = CreateOrderDto {
        latitude: Some(999.0),
        longitude: Some(-9999.0),
        ..d
    };
    assert!(
        d.sanitize().is_err(),
        "coordenadas absurdas deben rechazarse"
    );
}

#[test]
fn acepta_coordenadas_de_la_zona_de_entrega() {
    let d = dto(json!([{ "product_id": "abc", "quantity": 1 }]));
    let d = CreateOrderDto {
        latitude: Some(23.1136),
        longitude: Some(-82.3666),
        ..d
    };
    assert!(d.sanitize().is_ok());
}

#[test]
fn recorta_campos_demasiado_largos() {
    let d = dto(json!([{ "product_id": "abc", "quantity": 1 }]));
    let d = CreateOrderDto {
        customer_name: "n".repeat(500),
        notes: Some("x".repeat(900)),
        ..d
    };
    let out = d.sanitize().expect("debe recortar, no rechazar");
    assert_eq!(out.customer_name.chars().count(), 120);
    assert_eq!(out.notes.unwrap().chars().count(), 300);
}

#[test]
fn phone_e_id_number_son_opcionales() {
    // El frontend no los enviaba, y antes causaban 422.
    let payload = json!({
        "customer_name": "Cliente",
        "delivery_address": "Calle 1",
        "items": [{ "product_id": "abc", "quantity": 1 }],
    });
    let d: CreateOrderDto = serde_json::from_value(payload).expect("debe deserializar");
    let d = d.sanitize().expect("debe sanitizar");
    assert_eq!(d.customer_phone, "");
    assert_eq!(d.customer_id_number, "");
}
