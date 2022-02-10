FROM gcr.io/distroless/static:nonroot
COPY --chown=nonroot:nonroot ./hahaha /app/
EXPOSE 8999
ENTRYPOINT ["/app/hahaha"]