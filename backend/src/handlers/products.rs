use std::error::Error as _;

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use uuid::Uuid;

use crate::handlers::auth::AdminAuth;
use crate::models::product::{
    validate_name, validate_price, CreateProductDto, Product, ProductImage, UpdateProductDto,
};
use crate::state::AppState;

pub const PRODUCT_COLS: &str =
    "id, name, description, price, category, available, created_at, image";

/// Tope real de subida. Axum aplica `DefaultBodyLimit` (2 MB por defecto),
/// muy por debajo de lo que anuncia la interfaz (5 MB).
const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

/// Tope del cuerpo de la petición de subida: la imagen más el sobre de
/// multipart. Si se supera, se rechaza antes de leer nada.
const MAX_REQUEST_BYTES: usize = MAX_IMAGE_BYTES + 512 * 1024;

/// Devuelve el nombre del archivo de la imagen del producto, si tiene.
async fn fetch_image_filename(
    db: &sea_orm::DatabaseConnection,
    id: &str,
) -> Result<Option<String>, DbErrLike> {
    let sql = "SELECT image FROM products WHERE id = ?";
    let stmt = Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql, vec![id.into()]);
    let found = ProductImage::find_by_statement(stmt)
        .one(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(found.and_then(|p| p.image))
}

type DbErrLike = String;

fn uploads_dir() -> String {
    std::env::var("UPLOADS_DIR").unwrap_or_else(|_| "uploads".to_string())
}

/// Borra el archivo de una imagen del disco, ignorando errores.
fn remove_image_file(filename: &str) {
    let dir = uploads_dir();
    // Defensa extra: nunca construir una ruta fuera del directorio de subidas.
    if filename.contains('/') || filename.contains("..") || filename.is_empty() {
        return;
    }
    let path = std::path::Path::new(&dir).join(filename);
    let _ = std::fs::remove_file(path);
}

pub async fn list_products(
    State(state): State<AppState>,
) -> Result<Json<Vec<Product>>, (StatusCode, String)> {
    let sql = format!("SELECT {} FROM products ORDER BY name ASC", PRODUCT_COLS);
    let stmt = Statement::from_string(DatabaseBackend::Sqlite, sql);

    let products = Product::find_by_statement(stmt)
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(products))
}

pub async fn get_product(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Product>, (StatusCode, String)> {
    let sql = format!("SELECT {} FROM products WHERE id = ?", PRODUCT_COLS);
    let stmt = Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql, vec![id.into()]);

    let product = Product::find_by_statement(stmt)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Producto no encontrado".to_string()))?;

    Ok(Json(product))
}

pub async fn create_product(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(payload): Json<CreateProductDto>,
) -> Result<(StatusCode, Json<Product>), (StatusCode, String)> {
    let name = payload.name.trim().to_string();
    if let Err(msg) = validate_name(&name) {
        return Err((StatusCode::BAD_REQUEST, msg));
    }
    if let Err(msg) = validate_price(payload.price) {
        return Err((StatusCode::BAD_REQUEST, msg));
    }

    let id = Uuid::new_v4().to_string();
    let description = payload.description.unwrap_or_default().trim().to_string();
    let category = payload
        .category
        .unwrap_or_else(|| "Pizza".to_string())
        .trim()
        .to_string();
    let available = payload.available.unwrap_or(true);
    let created_at = Utc::now().to_rfc3339();

    let sql = "INSERT INTO products (id, name, description, price, category, available, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)";
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql,
        vec![
            id.clone().into(),
            name.clone().into(),
            description.clone().into(),
            payload.price.into(),
            category.clone().into(),
            available.into(),
            created_at.clone().into(),
        ],
    );

    state
        .db
        .execute(stmt)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(Product {
            id,
            name,
            description,
            price: payload.price,
            category,
            available,
            created_at,
            image: None,
        }),
    ))
}

pub async fn update_product(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateProductDto>,
) -> Result<Json<Product>, (StatusCode, String)> {
    let sql_select = format!("SELECT {} FROM products WHERE id = ?", PRODUCT_COLS);
    let stmt_select = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql_select,
        vec![id.clone().into()],
    );

    let existing = Product::find_by_statement(stmt_select)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Producto no encontrado".to_string()))?;

    let name = match payload.name {
        Some(ref n) => {
            let trimmed = n.trim().to_string();
            if let Err(msg) = validate_name(&trimmed) {
                return Err((StatusCode::BAD_REQUEST, msg));
            }
            trimmed
        }
        None => existing.name.clone(),
    };

    let price = payload.price.unwrap_or(existing.price);
    if let Err(msg) = validate_price(price) {
        return Err((StatusCode::BAD_REQUEST, msg));
    }

    let description = payload
        .description
        .map(|d| d.trim().to_string())
        .unwrap_or(existing.description);
    let category = payload
        .category
        .map(|c| c.trim().to_string())
        .unwrap_or(existing.category);
    let available = payload.available.unwrap_or(existing.available);

    // Quitar la imagen: se borra el archivo y se limpia la columna.
    let previous_image = if payload.clear_image && existing.image.is_some() {
        remove_image_file(existing.image.as_deref().unwrap());
        None
    } else {
        existing.image.clone()
    };

    let sql_update = "UPDATE products SET name = ?, description = ?, price = ?, category = ?, available = ?, image = ? WHERE id = ?";
    let stmt_update = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql_update,
        vec![
            name.clone().into(),
            description.clone().into(),
            price.into(),
            category.clone().into(),
            available.into(),
            previous_image.clone().into(),
            id.clone().into(),
        ],
    );

    state
        .db
        .execute(stmt_update)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(Product {
        id,
        name,
        description,
        price,
        category,
        available,
        created_at: existing.created_at,
        image: previous_image,
    }))
}

pub async fn delete_product(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let image_file = fetch_image_filename(&state.db, &id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let sql = "DELETE FROM products WHERE id = ?";
    let stmt = Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql, vec![id.into()]);

    let res = state
        .db
        .execute(stmt)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if res.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Producto no encontrado".to_string()));
    }

    if let Some(filename) = image_file {
        remove_image_file(&filename);
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Detecta el formato real del archivo por sus bytes iniciales.
/// El `Content-Type` declarado por el cliente no es de fiar.
fn sniff_image_extension(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if data.len() > 12 && data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("gif")
    } else if data.len() > 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

pub async fn upload_image(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<Product>, (StatusCode, String)> {
    // Se comprueba `Content-Length` antes de tocar el multipart: si la subida
    // es más grande que el tope del cuerpo, axum la cortaría con un error
    // interno de parseo que el admin no entendería.
    let too_large = || {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "La imagen no puede superar {} MB",
                MAX_IMAGE_BYTES / (1024 * 1024)
            ),
        )
    };

    if let Some(len) = headers
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
    {
        if len > MAX_REQUEST_BYTES {
            tracing::warn!("Subida rechazada: {} bytes superan el máximo", len);
            return Err(too_large());
        }
    }

    let sql_select = format!("SELECT {} FROM products WHERE id = ?", PRODUCT_COLS);
    let stmt_select = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql_select,
        vec![id.clone().into()],
    );

    let existing = Product::find_by_statement(stmt_select)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Producto no encontrado".to_string()))?;

    let dir = uploads_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut image_filename: Option<String> = None;

    // Red de seguridad: si multipart se rompe, el mensaje menciona el tamaño,
    // que es la causa más habitual, en lugar de dejar un error interno.
    let map_multipart_error = |e: axum::extract::multipart::MultipartError| {
        let is_too_large = e
            .source()
            .and_then(|s| s.downcast_ref::<multer::Error>())
            .map(|m| matches!(m, multer::Error::StreamSizeExceeded { .. }))
            .unwrap_or(false);

        if is_too_large {
            too_large()
        } else {
            (
                StatusCode::BAD_REQUEST,
                format!(
                    "No se pudo leer el archivo (¿supera {} MB?): {}",
                    MAX_IMAGE_BYTES / (1024 * 1024),
                    e
                ),
            )
        }
    };

    while let Some(field) = multipart.next_field().await.map_err(map_multipart_error)? {
        if field.name().unwrap_or_default() != "image" {
            continue;
        }

        let data = field.bytes().await.map_err(map_multipart_error)?;

        if data.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "El archivo está vacío".to_string()));
        }
        if data.len() > MAX_IMAGE_BYTES {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "La imagen no puede superar {} MB",
                    MAX_IMAGE_BYTES / (1024 * 1024)
                ),
            ));
        }

        let ext = sniff_image_extension(&data).ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "El archivo no es una imagen válida (use JPG, PNG, WebP o GIF)".to_string(),
            )
        })?;

        let filename = format!("{}.{}", Uuid::new_v4(), ext);
        std::fs::write(std::path::Path::new(&dir).join(&filename), &data)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        image_filename = Some(filename);
    }

    let filename = image_filename.ok_or((
        StatusCode::BAD_REQUEST,
        "No se envió ninguna imagen".to_string(),
    ))?;

    let sql_update = "UPDATE products SET image = ? WHERE id = ?";
    let stmt_update = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql_update,
        vec![filename.clone().into(), id.clone().into()],
    );
    state
        .db
        .execute(stmt_update)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Reemplazar la imagen no debe dejar archivos huérfanos en el disco.
    if let Some(previous) = existing.image.as_deref() {
        if previous != filename {
            remove_image_file(previous);
        }
    }

    Ok(Json(Product {
        id: existing.id,
        name: existing.name,
        description: existing.description,
        price: existing.price,
        category: existing.category,
        available: existing.available,
        created_at: existing.created_at,
        image: Some(filename),
    }))
}
