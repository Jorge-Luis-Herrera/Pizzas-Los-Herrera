use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use uuid::Uuid;
use chrono::Utc;

use crate::handlers::auth::AdminAuth;
use crate::models::product::{CreateProductDto, Product, UpdateProductDto};
use crate::state::AppState;

const PRODUCT_COLS: &str = "id, name, description, price, category, available, created_at, image";

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
    let id = Uuid::new_v4().to_string();
    let description = payload.description.unwrap_or_default();
    let category = payload.category.unwrap_or_else(|| "Pizza".to_string());
    let available = payload.available.unwrap_or(true);
    let created_at = Utc::now().to_rfc3339();

    let sql = "INSERT INTO products (id, name, description, price, category, available, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)";
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql,
        vec![
            id.clone().into(),
            payload.name.clone().into(),
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
            name: payload.name,
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

    let name = payload.name.unwrap_or(existing.name);
    let description = payload.description.unwrap_or(existing.description);
    let price = payload.price.unwrap_or(existing.price);
    let category = payload.category.unwrap_or(existing.category);
    let available = payload.available.unwrap_or(existing.available);

    let sql_update = "UPDATE products SET name = ?, description = ?, price = ?, category = ?, available = ? WHERE id = ?";
    let stmt_update = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql_update,
        vec![
            name.clone().into(),
            description.clone().into(),
            price.into(),
            category.clone().into(),
            available.into(),
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
        image: existing.image,
    }))
}

pub async fn delete_product(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let sql_select = format!("SELECT image FROM products WHERE id = ?");
    let stmt_select = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql_select,
        vec![id.clone().into()],
    );
    let image_file: Option<String> = <Product as sea_orm::FromQueryResult>::find_by_statement(stmt_select)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .and_then(|p| p.image);

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
        let path = format!("uploads/{}", filename);
        let _ = std::fs::remove_file(&path);
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn upload_image(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    mut multipart: Multipart,
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

    std::fs::create_dir_all("uploads")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut image_filename: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        let name = field.name().unwrap_or_default().to_string();
        if name != "image" {
            continue;
        }

        let content_type = field
            .content_type()
            .map(|m| m.to_string())
            .unwrap_or_default();

        let ext = match content_type.as_str() {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            "image/gif" => "gif",
            _ => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Formato de imagen no soportado (use jpg, png, webp o gif)".to_string(),
                ));
            }
        };

        let filename = format!("{}.{}", Uuid::new_v4(), ext);
        let filepath = format!("uploads/{}", filename);

        let data = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

        if data.len() > 5 * 1024 * 1024 {
            return Err((
                StatusCode::BAD_REQUEST,
                "La imagen no puede superar 5MB".to_string(),
            ));
        }

        std::fs::write(&filepath, &data)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        image_filename = Some(filename);
    }

    let filename = image_filename
        .ok_or((StatusCode::BAD_REQUEST, "No se envió ninguna imagen".to_string()))?;

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
