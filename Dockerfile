###############################################################################
## Builder
###############################################################################
FROM rust:alpine3.23 AS builder
RUN apk add --update --no-cache \
    autoconf \
    gcc \
    make \
    musl-dev \
    musl-utils \
    openssl \
    openssl-dev \
    openssl-libs-static \
    pkgconfig \
    binaryen \
    && rm -rf /var/cache/apk

WORKDIR /app

# Cache dependencies (avoid recompiling every time)
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY core/Cargo.toml core/
COPY backend/Cargo.toml backend/
COPY frontend/Cargo.toml frontend/

RUN mkdir -p core/src backend/src frontend/src && \
    echo "fn main() {}" > backend/src/main.rs && \
    echo "" > core/src/lib.rs && \
    echo "" > frontend/src/lib.rs

# Add wasm32 target and install wasm-bindgen (matching Cargo.lock version)
RUN rustup target add wasm32-unknown-unknown && \
    cargo install wasm-bindgen-cli --version 0.2.126

# Cache backend and frontend dependencies
RUN cargo build --release -p oxinbox-backend 2>/dev/null; true
RUN cargo build --release --target wasm32-unknown-unknown -p oxinbox-frontend 2>/dev/null; true

# Real source — backend
COPY core/src/ core/src/
COPY backend/src/ backend/src/
COPY backend/migrations/ backend/migrations/

ENV OPENSSL_LIB_DIR=/usr/lib \
    OPENSSL_STATIC=1 \
    SQLX_OFFLINE=true

RUN cargo build --release -p oxinbox-backend && \
    strip target/release/oxinbox-backend

# Real source — frontend
COPY frontend/src/ frontend/src/
COPY frontend/Dioxus.toml frontend/

RUN cargo build --release --target wasm32-unknown-unknown -p oxinbox-frontend && \
    mkdir -p frontend/dist/assets && \
    wasm-bindgen \
        --target web \
        --out-dir /app/frontend/dist/assets \
        target/wasm32-unknown-unknown/release/oxinbox_frontend.wasm && \
    wasm-opt -Oz \
        /app/frontend/dist/assets/oxinbox_frontend_bg.wasm \
        -o /app/frontend/dist/assets/oxinbox_frontend_bg.wasm 2>/dev/null || true

# Assemble complete dist with static files
COPY frontend/dist/index.html frontend/dist/
COPY frontend/public/ frontend/dist/

###############################################################################
## Final image
###############################################################################
FROM alpine:3.23

ENV USER=app \
    UID=1000

RUN apk add --update --no-cache \
    ca-certificates \
    wget \
    && rm -rf /var/cache/apk && \
    adduser \
        --disabled-password \
        --gecos "" \
        --home "/${USER}" \
        --shell "/sbin/nologin" \
        --uid "${UID}" \
        "${USER}"

COPY --from=builder /app/target/release/oxinbox-backend /app/oxinbox-backend
COPY --from=builder /app/backend/migrations /app/migrations/
COPY --from=builder /app/frontend/dist /app/frontend/dist/

RUN chown -R app:app /app

WORKDIR /app
USER app
EXPOSE 3300
CMD ["/app/oxinbox-backend"]