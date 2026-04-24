FROM rust:1.83-alpine AS build
RUN apk add --no-cache musl-dev pkgconf openssl-dev openssl-libs-static zlib-dev zlib-static cmake make gcc g++ libssh2-dev libssh2-static
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY tests ./tests
RUN cargo build --release --locked
RUN strip target/release/mcp-server-git-rs

FROM alpine:3.20
RUN apk add --no-cache git ca-certificates openssh-client
COPY --from=build /src/target/release/mcp-server-git-rs /usr/local/bin/mcp-server-git-rs
ENTRYPOINT ["mcp-server-git-rs"]
