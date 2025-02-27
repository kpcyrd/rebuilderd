FROM rust:alpine3.21
ENV RUSTFLAGS="-C target-feature=-crt-static"
WORKDIR /usr/src/rebuilderd
RUN apk add --no-cache musl-dev shared-mime-info sqlite-dev xz-dev zstd-dev
COPY . .
RUN --mount=type=cache,target=/var/cache/buildkit \
    CARGO_HOME=/var/cache/buildkit/cargo \
    CARGO_TARGET_DIR=/var/cache/buildkit/target \
    cargo build --release --locked -p rebuilderd -p rebuildctl && \
    cp -v /var/cache/buildkit/target/release/rebuilderd \
        /var/cache/buildkit/target/release/rebuildctl /

FROM alpine:3.21
RUN apk add --no-cache libgcc shared-mime-info sqlite-libs xz zstd-libs
COPY --from=0 \
    /rebuilderd /rebuildctl \
    /usr/local/bin/
ENV HTTP_ADDR=0.0.0.0:8484
VOLUME ["/data", "/secret"]
WORKDIR /data
CMD ["rebuilderd"]
