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

WORKDIR /app
COPY --from=builder /build/backend/target/release/pizzeria-api /app/pizzeria-api
COPY frontend /app/frontend
RUN mkdir -p /app/uploads

ENV PORT=8080
ENV RUST_LOG=info
EXPOSE 8080

CMD ["/app/pizzeria-api"]
