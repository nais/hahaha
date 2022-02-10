FROM gcr.io/distroless/static:nonroot
COPY --chown=nonroot:nonroot ./hahaha /app/
EXPOSE 8999
ENV RUST_LOG=info,kube=warn
ENTRYPOINT ["/app/hahaha"]