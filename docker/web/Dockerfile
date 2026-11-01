# AlwyChat web version: static files served by Caddy on port 80 (put it behind your reverse
# proxy for https, like Element Web). There is no server program: the app runs in the browser.
#
#   docker build -f docker/web/Dockerfile -t alwychat-web .
#   docker run -p 8080:80 alwychat-web

FROM rust:1-bookworm AS build
RUN rustup target add wasm32-unknown-unknown \
 && curl -fsSL https://deb.nodesource.com/setup_24.x | bash - \
 && apt-get install -y --no-install-recommends nodejs \
 && npm install -g pnpm@10
WORKDIR /src
COPY . .
# wasm-bindgen's command-line tool must match the crate's version exactly.
RUN cargo install wasm-bindgen-cli --locked --version \
      "$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/^version = "\(.*\)"/\1/p')"
ARG ALWY_WEB_BASE=""
RUN cd app && pnpm install --frozen-lockfile && ALWY_WEB_BASE="$ALWY_WEB_BASE" pnpm build:web

FROM caddy:2-alpine
COPY docker/web/Caddyfile /etc/caddy/Caddyfile
COPY --from=build /src/app/build-web /srv
EXPOSE 80
