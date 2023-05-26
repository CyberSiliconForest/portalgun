# SPDX-FileCopyrightText: 2023 perillamint <perillamint@silicon.moe>
# SPDX-FileCopyrightText: 2020-2022 Alex Grinman <me@alexgr.in>
#
# SPDX-License-Identifier: MIT

FROM rust:alpine as builder

RUN apk add --no-cache openssl openssl-dev musl-dev

WORKDIR /portalgun
COPY . .

RUN cargo build --release --bin portalgun_moon

FROM alpine:latest 

RUN apk add --no-cache openssl

COPY --from=builder /portalgun/target/release/portalgun_moon /portalgun_moon

# client svc
EXPOSE 8080
# ctrl svc
EXPOSE 5000
# net svc
EXPOSE 10002

ENTRYPOINT ["/portalgun_moon"]
