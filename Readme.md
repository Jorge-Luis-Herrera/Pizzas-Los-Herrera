# Pizzería Los Herrera - Sistema de Ventas

## Descripción
- Sistema automatizado de ventas para pizzería Los Herrera con gestión de productos y entregas.

## Características
- CRUD de productos (protegido con login de admin)
- Gestión de pedidos (crear pedido público; listar/actualizar requiere admin)
- Sistema de ubicación (Google Maps)
- Integración con WhatsApp

## Requisitos y Tecnologías
- Rust 2021 (Edición actual)
- Axum (Framework Web ultraligero y asíncrono)
- SeaORM + SQLite (Base de datos embebida de alto rendimiento y bajo consumo)
- Frontend: Vanilla HTML5 + CSS3 + JavaScript (sin dependencias)

## Cómo Ejecutar

```bash
cd backend
cargo run
```

Para producción, define siempre una contraseña fuerte:

```bash
ADMIN_USER=admin ADMIN_PASSWORD='tu_clave_segura' PIZZERIA_WHATSAPP=53TUNUMERO cargo run
```

## Acceso

| Página | URL | Descripción |
|--------|-----|-------------|
| Cliente | `http://localhost:3000/` | Landing page para hacer pedidos por WhatsApp |
| Admin | `http://localhost:3000/admin` | Panel de administración (requiere login) |

## Configuración

| Variable | Default | Descripción |
|----------|---------|-------------|
| `ADMIN_USER` | `admin` | Usuario del panel |
| `ADMIN_PASSWORD` | `admin` | Contraseña del panel (cámbiala en producción) |
| `PIZZERIA_WHATSAPP` | `5350722776` | Número para enlaces `wa.me` |

El token de sesión de admin dura 12 horas y se guarda en `sessionStorage` del navegador.

## API (resumen)

- Públicas: `GET /api/products`, `POST /api/orders`, `POST /api/auth/login`
- Admin (header `Authorization: Bearer <token>`): mutaciones de productos, listado/detalle/estado de pedidos, `POST /api/auth/logout`
