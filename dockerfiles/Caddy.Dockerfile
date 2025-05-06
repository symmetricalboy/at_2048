##Builds the assets for the WASM app
FROM rust:1.80.0-bookworm AS wasm-builder
WORKDIR /app
COPY ../ /app
RUN rustup target add wasm32-unknown-unknown
RUN apt-get update && apt-get install -y pkg-config libssl-dev # Added for potential native deps for cargo-binstall or trunk
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall -y trunk wasm-bindgen-cli # Added -y for non-interactive
WORKDIR /app/app_2048
RUN trunk build --release --public-url /


FROM caddy:2.8.4-alpine AS caddy # Updated Caddy version
EXPOSE 80
EXPOSE 443
EXPOSE 443/udp
COPY ../production_configs/Caddyfile /etc/caddy/Caddyfile
COPY --from=wasm-builder /app/app_2048/dist /srv
COPY ../production_configs/client_metadata.json /srv/client_metadata.json