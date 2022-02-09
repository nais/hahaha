FROM alpine:3.15

COPY ./hahaha .

EXPOSE 8999
ENV RUST_LOG=info,kube=warn
CMD ["./hahaha"]