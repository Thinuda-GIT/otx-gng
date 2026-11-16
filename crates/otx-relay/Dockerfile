# OTX Relay container. Build from the repository root:
#   docker build -f crates/otx-relay/Dockerfile -t otx-relay .
# Run (config files in ./relay-config):
#   docker run -d --name otx-relay --restart unless-stopped -p 51888:51888/udp \
#     -v "$PWD/relay-config:/config:ro" otx-relay
FROM rust:1-slim AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p otx-relay

FROM debian:stable-slim
RUN useradd --system --no-create-home relay
COPY --from=build /src/target/release/otx-relay /usr/local/bin/otx-relay
USER relay
EXPOSE 51888/udp
ENTRYPOINT ["otx-relay", "--listen", "0.0.0.0:51888", "--clients", "/config/clients.txt", "--allow", "/config/allow.txt"]
