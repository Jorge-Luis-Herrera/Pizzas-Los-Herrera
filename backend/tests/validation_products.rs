//! Tests de validación de precio y nombre de producto.

use pizzeria_api::models::product::{validate_name, validate_price, MAX_PRICE, MIN_PRICE};

#[test]
fn rechaza_precio_negativo() {
    // Regresión: un precio negativo acababa en un pedido con total negativo
    // y en el mensaje de WhatsApp con "Total: $-200.00".
    assert!(validate_price(-100.0).is_err());
}

#[test]
fn rechaza_precio_cero() {
    assert!(validate_price(0.0).is_err());
}

#[test]
fn acepta_precio_valido() {
    assert!(validate_price(11.50).is_ok());
    assert!(validate_price(MIN_PRICE).is_ok());
}

#[test]
fn rechaza_precio_absurdo() {
    assert!(validate_price(MAX_PRICE + 1.0).is_err());
}

#[test]
fn rechaza_precio_no_finito() {
    assert!(validate_price(f64::NAN).is_err());
    assert!(validate_price(f64::INFINITY).is_err());
}

#[test]
fn rechaza_nombre_vacio() {
    assert!(validate_name("").is_err());
    assert!(validate_name("   ").is_err());
}

#[test]
fn acepta_nombre_valido() {
    assert!(validate_name("Pizza Jamón y Champñones").is_ok());
}

#[test]
fn rechaza_nombre_demasiado_largo() {
    let largo = "a".repeat(121);
    assert!(validate_name(&largo).is_err());
}
