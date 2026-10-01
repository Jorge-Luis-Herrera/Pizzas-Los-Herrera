use sea_orm::{
    ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, DbErr, FromQueryResult,
    Statement,
};

pub async fn init_db() -> Result<DatabaseConnection, DbErr> {
    // Asegurar que el archivo de base de datos o su directorio existan
    let db_url = "sqlite://pizzeria.db?mode=rwc";

    let db = Database::connect(db_url).await?;

    // Activar modo WAL (Write-Ahead Logging) para mejor rendimiento de lectura/escritura concurrente
    db.execute_unprepared("PRAGMA journal_mode=WAL;").await?;
    db.execute_unprepared("PRAGMA foreign_keys=ON;").await?;

    // Crear tablas de productos, pedidos e ítems de pedidos si no existen
    create_tables(&db).await?;
    migrate_schema(&db).await?;

    Ok(db)
}

async fn create_tables(db: &DatabaseConnection) -> Result<(), DbErr> {
    // Tabla de Productos
    let create_products_sql = r#"
    CREATE TABLE IF NOT EXISTS products (
        id TEXT PRIMARY KEY NOT NULL,
        name TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',
        price REAL NOT NULL,
        category TEXT NOT NULL DEFAULT 'Pizza',
        available BOOLEAN NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL
    );
    "#;

    // Tabla de Pedidos
    let create_orders_sql = r#"
    CREATE TABLE IF NOT EXISTS orders (
        id TEXT PRIMARY KEY NOT NULL,
        customer_name TEXT NOT NULL,
        customer_id_number TEXT NOT NULL DEFAULT '',
        customer_phone TEXT NOT NULL,
        delivery_address TEXT NOT NULL,
        latitude REAL,
        longitude REAL,
        status TEXT NOT NULL DEFAULT 'Pendiente',
        total_amount REAL NOT NULL DEFAULT 0.0,
        notes TEXT NOT NULL DEFAULT '',
        created_at TEXT NOT NULL
    );
    "#;

    // Tabla de Ítems de Pedidos
    let create_order_items_sql = r#"
    CREATE TABLE IF NOT EXISTS order_items (
        id TEXT PRIMARY KEY NOT NULL,
        order_id TEXT NOT NULL,
        product_id TEXT NOT NULL,
        product_name TEXT NOT NULL,
        quantity INTEGER NOT NULL,
        unit_price REAL NOT NULL,
        subtotal REAL NOT NULL,
        FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE CASCADE
    );
    "#;

    db.execute_unprepared(create_products_sql).await?;
    db.execute_unprepared(create_orders_sql).await?;
    db.execute_unprepared(create_order_items_sql).await?;

    Ok(())
}

/// Añade columnas nuevas a tablas ya existentes (CREATE TABLE IF NOT EXISTS no las altera).
async fn migrate_schema(db: &DatabaseConnection) -> Result<(), DbErr> {
    if !column_exists(db, "orders", "customer_id_number").await? {
        db.execute_unprepared(
            "ALTER TABLE orders ADD COLUMN customer_id_number TEXT NOT NULL DEFAULT ''",
        )
        .await?;
        println!("📦 Migración: columna orders.customer_id_number añadida");
    }
    if !column_exists(db, "products", "image").await? {
        db.execute_unprepared(
            "ALTER TABLE products ADD COLUMN image TEXT DEFAULT NULL",
        )
        .await?;
        println!("📦 Migración: columna products.image añadida");
    }
    Ok(())
}

async fn column_exists(
    db: &DatabaseConnection,
    table: &str,
    column: &str,
) -> Result<bool, DbErr> {
    #[derive(Debug, sea_orm::FromQueryResult)]
    struct ColumnInfo {
        name: String,
    }

    let sql = format!("PRAGMA table_info({})", table);
    let stmt = Statement::from_string(DatabaseBackend::Sqlite, sql);
    let cols = ColumnInfo::find_by_statement(stmt).all(db).await?;
    Ok(cols.iter().any(|c| c.name == column))
}
