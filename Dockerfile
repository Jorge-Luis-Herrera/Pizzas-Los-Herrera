FROM rust:1.88-bookworm AS builder

WORKDIR /build
COPY backend/Cargo.toml backend/Cargo.lock ./backend/
COPY backend/src ./backend/src

WORKDIR /build/backend
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Ejecutar como usuario sin privilegios: el proceso solo necesita leer el
# frontend y escribir en /app/uploads y en la base de datos.
RUN groupadd --system pizzeria \
    && useradd --system --gid pizzeria --home /app --shell /usr/sbin/nologin pizzeria

WORKDIR /app
COPY --from=builder /build/backend/target/release/pizzeria-api /app/pizzeria-api
COPY frontend /app/frontend

RUN mkdir -p /app/uploads /app/data \
    && chown -R pizzeria:pizzeria /app

USER pizzeria

ENV PORT=8080
ENV RUST_LOG=info
# La base y las subidas viven en un volumen montado en producción.
ENV DATABASE_URL="sqlite:///app/data/pizzeria.db?mode=rwc"
ENV UPLOADS_DIR="/app/uploads"
EXPOSE 8080

CMD ["/app/pizzeria-api"]