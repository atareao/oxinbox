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
RUN cargo build --release -p oxinbox-backend 2>/dev/null; true

# Real source
COPY core/src/ core/src/
COPY backend/src/ backend/src/
COPY backend/migrations/ backend/migrations/

ENV OPENSSL_LIB_DIR=/usr/lib \
    OPENSSL_STATIC=1 \
    SQLX_OFFLINE=true

RUN cargo build --release -p oxinbox-backend && \
    strip target/release/oxinbox-backend

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
COPY frontend/dist /app/frontend/dist/
COPY frontend/public /app/frontend/dist/

RUN chown -R app:app /app

WORKDIR /app
USER app
EXPOSE 3300
CMD ["/app/oxinbox-backend"]