# Tasklist

## Hecho (2026-10-02)

Auditoría completa del proyecto y corrección de los hallazgos.

### Seguridad
- [x] XSS almacenado en el panel: los botones ya no interpolan datos del pedido en atributos `onclick`; usan `data-id` y delegación de eventos
- [x] Comparación de contraseña en tiempo constante (`subtle`)
- [x] Rate limit de login: 5 intentos fallidos por IP+usuario cada 5 minutos
- [x] CORS restringido: por defecto no se permite ningún origen cruzado (`ALLOWED_ORIGINS`)
- [x] Validación de contenido real de las imágenes por magic bytes

### Corrección de errores
- [x] `DELETE /api/products/:id` devolvía 500 (`SELECT image` deserializado en el struct `Product`)
- [x] Límite real de subida: 5 MB (antes el tope de axum era 2 MB y la UI prometía 5)
- [x] Precios negativos y absurdos rechazados
- [x] Límite de 50 unidades por producto y 30 productos por pedido
- [x] Campos obligatorios del pedidos validados y acotados en longitud
- [x] Coordenadas GPS validadas por rango y zona de reparto
- [x] URL de WhatsApp acotada para pedidos muy largos
- [x] Totales redondeados a 2 decimales

### Experiencia del cliente
- [x] Campo de teléfono obligatorio en el pedido (antes se enviaba siempre vacío)
- [x] El admin puede llamar y escribir por WhatsApp **al cliente**
- [x] `window.open` síncrono: la ventana de WhatsApp ya no la bloquea el navegador
- [x] Enlace de respaldo si el navegador bloquea la ventana
- [x] El total que devuelve el servidor es el definitivo; si cambia, se avisa
- [x] El carrito se revalida contra el menú (precios y disponibilidad)

### Operación
- [x] Actualización automática de pedidos cada 20 s, con aviso y sonido
- [x] Badge de pedidos nuevos sin revisar
- [x] `PATCH /api/orders/:id` para corregir dirección, teléfono y notas
- [x] Consulta de pedidos con un solo JOIN (eliminado el N+1)
- [x] Paginación y filtro por estado en `/api/orders`
- [x] Tokens firmados HMAC: la sesión sobrevive a reinicios y al escalado a cero
- [x] Logging estructurado con `tracing` + `RUST_LOG` funcional
- [x] Health check que consulta la base de datos
- [x] Cierre ordenado (SIGTERM/SIGINT)
- [x] Imágenes huérfanas eliminadas al reemplazarlas; opción de quitar la foto
- [x] Contenedor Docker con usuario sin privilegios

### Calidad
- [x] 36 tests (unitarios de validación + integración contra el router real)
- [x] CI con `fmt`, `clippy` y `test` antes de desplegar
- [x] Sin secretos ni placeholders en el repositorio
- [x] Documentación de `Readme.md` y `Step_By_Step.md` al día

## Pendiente / ideas

- [ ] Historial de cambios de estado por pedido (auditoría interna)
- [ ] Exportar pedidos a CSV para el cierre de caja
- [ ] Múltiples usuarios de admin con roles
- [ ] Método de pago (efectivo / transferencia) visible en el panel
- [ ] Horarios de apertura y comprobación automática antes de aceptar pedidos