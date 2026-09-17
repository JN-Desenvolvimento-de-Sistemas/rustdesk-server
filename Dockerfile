# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM rust:1.90-bookworm@sha256:3914072ca0c3b8aad871db9169a651ccfce30cf58303e5d6f2db16d1d8a7e58f AS build
RUN dpkg --add-architecture amd64 && apt-get update && apt-get install -y --no-install-recommends git pkg-config libssl-dev libsodium-dev protobuf-compiler gcc-x86-64-linux-gnu g++-x86-64-linux-gnu libssl-dev:amd64 libsodium-dev:amd64 && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-unknown-linux-gnu
WORKDIR /src
COPY . /src
ENV SODIUM_USE_PKG_CONFIG=1 CARGO_BUILD_JOBS=4
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git --mount=type=cache,target=/src/target cargo clean -p hbbs --release && cargo test --locked --release --lib adm::tests
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
    CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc \
    CXX_x86_64_unknown_linux_gnu=x86_64-linux-gnu-g++ \
    PKG_CONFIG_ALLOW_CROSS=1 \
    PKG_CONFIG_PATH_x86_64_unknown_linux_gnu=/usr/lib/x86_64-linux-gnu/pkgconfig \
    X86_64_UNKNOWN_LINUX_GNU_OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu \
    X86_64_UNKNOWN_LINUX_GNU_OPENSSL_INCLUDE_DIR=/usr/include
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git --mount=type=cache,target=/src/target cargo build --locked --release --target x86_64-unknown-linux-gnu --bin hbbs --bin hbbr && mkdir /out && cp target/x86_64-unknown-linux-gnu/release/hbbs target/x86_64-unknown-linux-gnu/release/hbbr /out/

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libsodium23 libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=build /out/hbbs /usr/local/bin/hbbs
COPY --from=build /out/hbbr /usr/local/bin/hbbr
WORKDIR /root
CMD ["hbbs"]
