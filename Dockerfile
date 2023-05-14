FROM rust:alpine as builder

RUN apk add --no-cache openssl openssl-dev musl-dev

WORKDIR /tunnelto
COPY . .

RUN cargo build --release --bin tunnelto_server

FROM alpine:latest 

RUN apk add --no-cache openssl

COPY --from=builder /tunnelto/target/release/tunnelto_server /tunnelto_server

# client svc
EXPOSE 8080
# ctrl svc
EXPOSE 5000
# net svc
EXPOSE 10002

ENTRYPOINT ["/tunnelto_server"]
