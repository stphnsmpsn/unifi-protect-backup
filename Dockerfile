# ---- Build Stage ----
FROM rust:1.88.0-slim-bullseye as build
ARG DEBIAN_FRONTEND=noninteractive
RUN apt update && apt install -y \
    iputils-ping \
    libpq-dev \
    cmake \
    pkg-config \
    gcc \
    g++ \
    python3 \
    libssl-dev \
    protobuf-compiler \
    git
ARG DEBIAN_FRONTEND=noninteractive
WORKDIR /app
COPY . .
RUN cargo build --release

## ---- Production Stage ----
FROM debian:bookworm AS production
WORKDIR /app
COPY --from=build /app/target/release/unifi-protect-backup ./
CMD [ "./unifi-protect-backup" ]
