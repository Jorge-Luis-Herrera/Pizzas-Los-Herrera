use serde::{Deserialize, Serialize};

/// Máximo de unidades del mismo producto en un pedido.
pub const MAX_ITEM_QUANTITY: i32 = 50;
/// Máximo de líneas distintas por pedido.
pub const MAX_ORDER_ITEMS: usize = 30;

pub const MAX_NAME_LEN: usize = 120;
pub const MAX_ID_NUMBER_LEN: usize = 20;
pub const MAX_PHONE_LEN: usize = 40;
pub const MAX_ADDRESS_LEN: usize = 400;
pub const MAX_NOTES_LEN: usize = 300;

fn clean(value: String, max: usize) -> String {
    value.trim().chars().take(max).collect()
}

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
    /// Opcional: si no se envía se guarda como cadena vacía.
    #[serde(default)]
    pub customer_id_number: String,
    /// Opcional: si no se envía se guarda como cadena vacía.
    #[serde(default)]
    pub customer_phone: String,
    pub delivery_address: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub notes: Option<String>,
    pub items: Vec<CreateOrderItemDto>,
}

impl CreateOrderDto {
    /// Normaliza y valida los datos de entrada del cliente.
    /// Devuelve un error legible en español ante cualquier dato inválido.
    pub fn sanitize(mut self) -> Result<Self, String> {
        self.customer_name = clean(self.customer_name, MAX_NAME_LEN);
        self.customer_id_number = clean(self.customer_id_number, MAX_ID_NUMBER_LEN);
        self.customer_phone = clean(self.customer_phone, MAX_PHONE_LEN);
        self.delivery_address = clean(self.delivery_address, MAX_ADDRESS_LEN);
        self.notes = Some(clean(self.notes.unwrap_or_default(), MAX_NOTES_LEN));

        if self.customer_name.is_empty() {
            return Err("El nombre del cliente es obligatorio".to_string());
        }
        if self.delivery_address.is_empty() {
            return Err("La dirección de entrega es obligatoria".to_string());
        }
        if self.items.is_empty() {
            return Err("El pedido debe contener al menos un producto".to_string());
        }
        if self.items.len() > MAX_ORDER_ITEMS {
            return Err(format!(
                "Un pedido no puede tener más de {} productos distintos",
                MAX_ORDER_ITEMS
            ));
        }

        if let (Some(lat), Some(lng)) = (self.latitude, self.longitude) {
            if !(-90.0..=90.0).contains(&lat) {
                return Err("La latitud no es válida".to_string());
            }
            if !(-180.0..=180.0).contains(&lng) {
                return Err("La longitud no es válida".to_string());
            }
            // Un par de coordenadas fuera de este rango es casi seguro un error
            // de dedo o un GPS corrupto, y genera enlaces de mapa inservibles.
            if !(14.0..=24.5).contains(&lat) || !(-90.0..=-66.0).contains(&lng) {
                return Err(
                    "Las coordenadas GPS no parecen válidas para la zona de entrega".to_string(),
                );
            }
        }

        // Consolidar líneas duplicadas del mismo producto y validar cantidades.
        let mut merged: Vec<(String, i32)> = Vec::new();
        for item in &self.items {
            if item.quantity <= 0 {
                return Err("La cantidad de cada producto debe ser mayor que 0".to_string());
            }
            if item.quantity > MAX_ITEM_QUANTITY {
                return Err(format!(
                    "Máximo {} unidades por producto",
                    MAX_ITEM_QUANTITY
                ));
            }
            if item.product_id.trim().is_empty() {
                return Err("Hay un producto sin identificador en el pedido".to_string());
            }
            match merged.iter_mut().find(|(id, _)| *id == item.product_id) {
                Some((_, qty)) => *qty += item.quantity,
                None => merged.push((item.product_id.clone(), item.quantity)),
            }
        }
        for (_, qty) in &merged {
            if *qty > MAX_ITEM_QUANTITY {
                return Err(format!(
                    "Máximo {} unidades por producto",
                    MAX_ITEM_QUANTITY
                ));
            }
        }

        self.items = merged
            .into_iter()
            .map(|(product_id, quantity)| CreateOrderItemDto {
                product_id,
                quantity,
            })
            .collect();
        Ok(self)
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrderStatusDto {
    pub status: String,
}

/// Permite corregir los datos de entrega de un pedido ya creado
/// (dirección mal escrita, teléfono, notas). No toca los ítems ni el total.
#[derive(Debug, Deserialize)]
pub struct UpdateOrderDto {
    pub customer_name: Option<String>,
    pub customer_id_number: Option<String>,
    pub customer_phone: Option<String>,
    pub delivery_address: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub notes: Option<String>,
}
