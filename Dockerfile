FROM node:24-bookworm-slim AS frontend
WORKDIR /build/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

FROM rust:1.98-bookworm AS backend
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY backend/ ./backend/
RUN cargo build --locked --release --workspace

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates gosu \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 melody && useradd --uid 10001 --gid melody --no-create-home melody
WORKDIR /app
COPY --from=backend /build/target/release/melody-path-api /app/melody-path-api
COPY --from=frontend /build/frontend/dist/ /app/static/
COPY deploy/entrypoint.sh /app/entrypoint.sh
RUN chmod 755 /app/entrypoint.sh
ENV MELODYPATH_ENV=production \
    MELODYPATH_STATIC_DIR=/app/static \
    MELODYPATH_DATA_DIR=/var/data/melodypath \
    OAUTH_TOKEN_STORE=server_encrypted \
    OAUTH_COOKIE_SECURE=true
EXPOSE 10000
ENTRYPOINT ["/app/entrypoint.sh"]
