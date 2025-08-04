# ---- Build Stage ----
# This stage builds the application using the rust-builder image
FROM gitlab-registry.stephensampson.dev/stphnsmpsn/ci-templates/rust-builder:latest AS build

WORKDIR /app
COPY . .
RUN cargo build --release

# ---- Production Release Image ----
# This image is used to run the service binary.
# It is built on top of the template base image and adds the service binary.
FROM gitlab-registry.stephensampson.dev/stphnsmpsn/ci-templates/rust-base:latest AS release

COPY --from=build /app/target/release/unifi-protect-backup ./
USER app
CMD [ "./unifi-protect-backup" ]