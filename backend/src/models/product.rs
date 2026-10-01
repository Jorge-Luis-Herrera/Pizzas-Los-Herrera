use serde::{Deserialize, Serialize};

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
}
