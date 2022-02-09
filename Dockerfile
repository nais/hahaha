FROM gcr.io/distroless/static:nonroot
COPY --chown=nonroot:nonroot ./hahaha /app/
EXPOSE 8080
ENTRYPOINT ["/app/hahaha"]