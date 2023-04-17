FROM ekidd/rust-musl-builder:latest as builder
WORKDIR /build
COPY . .
ENV RUSTFLAGS='-C target-feature=+crt-static'
RUN cargo test --release --target x86_64-unknown-linux-gnu -- --test-threads 1
RUN cargo build --release

FROM gcr.io/distroless/static-debian11:nonroot
WORKDIR /app
COPY --from=builder /build/target/release/hahaha /app/hahaha
EXPOSE 8999
ENV RUST_LOG="hahaha=debug,kube=warn"
ENTRYPOINT ["/app/hahaha"]

