# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM rust:1.91.1-bookworm as builder

ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
ENV CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
ENV CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++

WORKDIR /app/builder
COPY . ./

ARG TARGETPLATFORM
RUN <<-EOF
    case "$TARGETPLATFORM" in
      "linux/arm64")
        apt-get update && apt-get install -qq g++-aarch64-linux-gnu libc6-dev-arm64-cross
        echo aarch64-unknown-linux-gnu > /rust_target.txt ;;
      "linux/amd64")
        echo x86_64-unknown-linux-gnu > /rust_target.txt ;;
      *)
        exit 1 ;;
    esac
EOF

RUN rustup target add $(cat /rust_target.txt)
RUN cargo build --release --target $(cat /rust_target.txt)
RUN cp ./target/$(cat /rust_target.txt)/release/spider-bot /spider-bot

FROM gcr.io/distroless/cc-debian12 as application

COPY --from=builder /spider-bot /

EXPOSE 8000
ENTRYPOINT ["./spider-bot"]
