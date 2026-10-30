# Builds the Grandspace server and the browser client, then ships both in a
# small image. The server serves the web client at / and the game at /ws.

FROM rust:1-bookworm AS build
RUN rustup target add wasm32-unknown-unknown
WORKDIR /src
# Faster, lighter builds than the release profile in Cargo.toml (which is
# tuned for desktop binaries).
ENV CARGO_PROFILE_RELEASE_LTO=false \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
    CARGO_TERM_COLOR=never
COPY . .
RUN cargo build --release -p grandspace-server
RUN cargo build --release --target wasm32-unknown-unknown -p grandspace-client
# wasm-bindgen's command-line tool must match the library version exactly.
RUN WBG=$(awk '/^name = "wasm-bindgen"$/ { getline; print $3 }' Cargo.lock | tr -d '"') \
 && echo "wasm-bindgen $WBG" \
 && cargo install wasm-bindgen-cli --version "$WBG" --locked
RUN wasm-bindgen --target web --no-typescript --out-name grandspace --out-dir dist/pkg \
      target/wasm32-unknown-unknown/release/grandspace-client.wasm \
 && cp web/index.html dist/ \
 && gzip -9 -k dist/pkg/*

FROM debian:bookworm-slim
COPY --from=build /src/target/release/grandspace-server /usr/local/bin/grandspace-server
COPY --from=build /src/dist /srv/web
# One shared galaxy: the clock can't be paused by players. Listens on $PORT.
ENTRYPOINT ["grandspace-server", "--web-dir", "/srv/web", "--locked-clock", "--speed", "2"]
