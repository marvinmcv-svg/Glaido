# Vocalis gateway image. Build from the repository root:
#   docker build -f apps/gateway/Dockerfile -t vocalis-gateway .
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
# rust-toolchain.toml asks rustup for "stable" plus components; the image's
# stable toolchain is enough to build, so skip the extra download.
RUN rm -f rust-toolchain.toml \
 && cargo build --release --locked -p vocalis-gateway \
 && cp target/release/vocalis-gateway /vocalis-gateway

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 vocalis
COPY --from=build /vocalis-gateway /usr/local/bin/vocalis-gateway
USER vocalis
ENV RUST_LOG=info
# Railway (and most hosts) set PORT; the gateway defaults to 8787.
EXPOSE 8787
CMD ["vocalis-gateway"]
