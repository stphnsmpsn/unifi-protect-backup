# ---- Build Stage ----
# This stage builds the application using the rust-builder image
FROM gitlab-registry.stephensampson.dev/stphnsmpsn/ci-templates/rust-builder:latest AS build

WORKDIR /app
COPY . .
# hack to prevent QEMU internal SIGILL & QEMU internal SIGSEGV (may also be caused by ASLR)
RUN if [ "$(uname -m)" = "aarch64" ]; then export CARGO_BUILD_JOBS=4; fi && cargo build --release

# ---- Production Release Image ----
# This image is used to run the service binary.
# It is built on top of the template base image and adds the service binary.
FROM gitlab-registry.stephensampson.dev/stphnsmpsn/ci-templates/rust-base:latest AS release

ARG DEBIAN_FRONTEND=noninteractive
RUN apt update && apt install -y \
    borgbackup \
    rclone \
    openssh-client

COPY --from=build /app/target/release/unifi-protect-backup ./
USER app
CMD [ "./unifi-protect-backup" ]