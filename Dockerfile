###############################################################################
## Backend builder
###############################################################################
FROM rust:alpine3.23 AS backend-builder
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

COPY ./backend/ ./

ENV OPENSSL_LIB_DIR=/usr/lib \
    OPENSSL_STATIC=1 \
    SQLX_OFFLINE=true

RUN cargo build --release && \
    strip target/release/oxinbox

###############################################################################
## Frontend builder
###############################################################################
FROM node:23-alpine AS frontend-builder
ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
ENV CI=true
RUN corepack enable
WORKDIR /app/frontend
COPY frontend/package.json frontend/pnpm-lock.yaml frontend/pnpm-workspace.yaml ./
RUN --mount=type=cache,id=pnpm,target=/pnpm/store \
    pnpm install --frozen-lockfile
COPY frontend/ ./
RUN pnpm run build

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

COPY --from=backend-builder /app/target/release/oxinbox /app/oxinbox
COPY --from=backend-builder /app/migrations /app/migrations/
COPY --from=frontend-builder /app/frontend/dist /app/frontend/dist/

RUN chown -R app:app /app

WORKDIR /app
USER app
EXPOSE 3300
CMD ["/app/oxinbox"]
