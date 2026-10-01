use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sea_orm::FromQueryResult)]
pub struct Order {
    pub id: String,
    pub customer_name: String,
    pub customer_id_number: String,
    pub customer_phone: String,
    pub delivery_address: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub status: String,
    pub total_amount: f64,
    pub notes: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sea_orm::FromQueryResult)]
pub struct OrderItem {
    pub id: String,
    pub order_id: String,
    pub product_id: String,
    pub product_name: String,
    pub quantity: i32,
    pub unit_price: f64,
    pub subtotal: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderWithItems {
    #[serde(flatten)]
    pub order: Order,
    pub items: Vec<OrderItem>,
    pub google_maps_url: Option<String>,
    pub whatsapp_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateOrderItemDto {
    pub product_id: String,
    pub quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct CreateOrderDto {
    pub customer_name: String,
    pub customer_id_number: String,
    pub customer_phone: String,
    pub delivery_address: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub notes: Option<String>,
    pub items: Vec<CreateOrderItemDto>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrderStatusDto {
    pub status: String,
}
