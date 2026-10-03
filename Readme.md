# Pizzería Los Herrera - Sistema de Ventas

## Descripción

Sistema automatizado de ventas para pizzería Los Herrera con gestión de productos y entregas.
El cliente arma su pedido en la web, el sistema lo registra y genera el mensaje de WhatsApp
para el local; el administrador gestiona el menú y el estado de cada pedido.

## Características

- Menú público con categorías, carrito persistente e imágenes de producto.
- Pedido con captura de GPS, teléfono de contacto y notas de entrega.
- Integración con WhatsApp: la URL del chat se genera en el servidor.
- Enlace directo a Google Maps con la ubicación del cliente.
- Panel de administración protegido por token:
  - CRUD de productos, disponibilidad e imágenes.
  - Seguimiento de pedidos por estado y búsqueda por ID, nombre o carnet.
  - Aviso automático y sonido cuando entra un pedido nuevo.
  - Corrección de los datos de entrega de un pedido.

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
| `PORT` | `3000` | Puerto del servidor |
| `DATABASE_URL` | `sqlite://pizzeria.db?mode=rwc` | Ruta de la base SQLite |
| `UPLOADS_DIR` | `uploads` | Carpeta de imágenes de producto |
| `ALLOWED_ORIGINS` | *(vacío)* | Orígenes CORS permitidos, separados por comas. Vacío = ninguno (la app es same-origin) |
| `RUST_LOG` | `info` | Filtro de logs de `tracing` |

> El token de sesión dura 12 horas y se guarda en `sessionStorage`.
> El token va firmado (HMAC-SHA256) y lleva su propia caducidad, así que
> **sobrevive a reinicios y al escalado a cero del contenedor**.

## Límites aplicados

| Regla | Valor |
|-------|-------|
| Precio de producto | entre `0.01` y `100000` |
| Cantidad por producto | hasta 50 unidades |
| Productos distintos por pedido | hasta 30 |
| Tamaño de imagen | hasta 5 MB (jpg, png, webp o gif) |
| Intentos de login fallidos | 5 cada 5 minutos por IP+usuario |

Los totales se calculan **siempre** en el servidor a partir del precio almacenado
en la base de datos: el cliente nunca puede decidir cuánto paga.

## API (resumen)

### Públicas

| Método | Ruta | Descripción |
|--------|------|-------------|
| `GET` | `/api/health` | Estado del servicio (consulta la base de datos) |
| `GET` | `/api/products` | Listar productos |
| `GET` | `/api/products/:id` | Ver un producto |
| `POST` | `/api/orders` | Crear pedido |
| `POST` | `/api/auth/login` | Iniciar sesión |

### Admin (header `Authorization: Bearer <token>`)

| Método | Ruta | Descripción |
|--------|------|-------------|
| `POST` | `/api/products` | Crear producto |
| `PUT` | `/api/products/:id` | Actualizar producto (`clear_image` quita la foto) |
| `DELETE` | `/api/products/:id` | Eliminar producto |
| `POST` | `/api/products/:id/image` | Subir imagen (multipart, campo `image`) |
| `GET` | `/api/orders` | Listar pedidos (`?status=`, `?limit=`) |
| `GET` | `/api/orders/:id` | Ver un pedido |
| `PATCH` | `/api/orders/:id` | Corregir datos de entrega |
| `DELETE` | `/api/orders/:id` | Eliminar pedido |
| `PATCH` | `/api/orders/:id/status` | Cambiar estado |
| `POST` | `/api/auth/logout` | Cerrar sesión |

Estados válidos: `Pendiente`, `En Preparación`, `En Camino`, `Entregado`.

## Tests

```bash
cd backend
cargo test
```

Incluye tests unitarios de validación y tests de integración que montan el
router real contra una base SQLite temporal.

## Despliegue

Producción corre en un **Azure App Service for Containers** (plan B1) con la base
SQLite y las imágenes montadas en un Azure Files share, de modo que los datos
sobreviven a reinicios y despliegues.

- **Tienda:** `https://pizzas-los-herrera-web.azurewebsites.net`
- **Panel:** `https://pizzas-los-herrera-web.azurewebsites.net/admin`

> **Por qué App Service y no Container Apps:** esta suscripción de Azure está
> migrada a **entornos Express**, que no soportan Azure Files
> (`ExpressEnvironmentResourceNotSupported`). Como la app necesita
> almacenamiento persistente para SQLite y las imágenes, se usa App Service,
> que sí admite montajes de Azure Files.

### Deploy a Azure App Service

```bash
./deploy/deploy-appservice.sh
```

El script verifica `fmt`/`clippy`/`tests`, construye la imagen, la sube al ACR,
actualiza el Web App y comprueba el health check. Para saltar las pruebas:
`--skip-tests`. Para usar una clave SSH en vez de contraseña del ACR: `--ssh <ruta>`.

Las credenciales se leen del entorno o de `~/.azure-pizzeria.env` (nunca lo
subas al repositorio):

```bash
# ~/.azure-pizzeria.env  (chmod 600)
ACR_USERNAME=<salida de: az acr credential show -n cac7aa06a97facr --query username -o tsv>
ACR_PASSWORD=<salida de: az acr credential show -n cac7aa06a97facr --query 'passwords[0].value' -o tsv>
ADMIN_PASSWORD=<una contraseña fuerte>
PIZZERIA_WHATSAPP=5350722776
# Opcionales: ADMIN_USER=admin   ALLOWED_ORIGINS=
```

### Deploy a VPS Rocky Linux

```bash
./deploy/build-and-deploy.sh <IP_SERVIDOR> <USUARIO_SSH>
```

Copia el binario y el frontend, instala el servicio systemd y nginx.
Las credenciales van en `/etc/pizzeria/pizzeria.env` (ver `deploy/pizzeria.service`).

### Docker

```bash
docker build -t pizzeria-api .
docker run -p 8080:8080 \
  -e ADMIN_USER=admin \
  -e ADMIN_PASSWORD='clave_fuerte' \
  -e PIZZERIA_WHATSAPP=5350722776 \
  -v pizzeria-data:/app/data \
  -v pizzeria-uploads:/app/uploads \
  pizzeria-api
```