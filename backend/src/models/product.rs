use serde::{Deserialize, Serialize};

/// Precio mínimo y máximo admitidos (en la moneda del local).
/// Evita precios negativos o absurdos que romperían el total de los pedidos.
pub const MIN_PRICE: f64 = 0.01;
pub const MAX_PRICE: f64 = 100_000.0;

pub fn validate_price(price: f64) -> Result<(), String> {
    if !price.is_finite() {
        return Err("El precio debe ser un número válido".to_string());
    }
    if price < MIN_PRICE {
        return Err(format!(
            "El precio debe ser mayor o igual a {:.2}",
            MIN_PRICE
        ));
    }
    if price > MAX_PRICE {
        return Err(format!("El precio no puede superar {:.2}", MAX_PRICE));
    }
    Ok(())
}

pub fn validate_name(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("El nombre del producto no puede estar vacío".to_string());
    }
    if trimmed.chars().count() > 120 {
        return Err("El nombre no puede superar los 120 caracteres".to_string());
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, sea_orm::FromQueryResult)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub description: String,
    pub price: f64,
    pub category: String,
    pub available: bool,
    pub created_at: String,
    pub image: Option<String>,
}

/// Proyección mínima para consultar sólo la imagen de un producto.
/// `SELECT image` NO se puede deserializar en `Product`, porque el derive
/// `FromQueryResult` exige que estén presentes todas sus columnas.
#[derive(Debug, Clone, sea_orm::FromQueryResult)]
pub struct ProductImage {
    pub image: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProductDto {
    pub name: String,
    pub description: Option<String>,
    pub price: f64,
    pub category: Option<String>,
    pub available: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProductDto {
    pub name: Option<String>,
    pub description: Option<String>,
    pub price: Option<f64>,
    pub category: Option<String>,
    pub available: Option<bool>,
    /// Poner en `true` para quitar la imagen del producto.
    #[serde(default)]
    pub clear_image: bool,
}
