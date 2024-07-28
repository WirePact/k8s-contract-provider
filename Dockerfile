FROM rust:1.80-alpine as build

ARG BUILD_VERSION=0.0.0-development

RUN apk add --update --no-cache openssl-dev musl-dev protoc
RUN rustup component add rustfmt

WORKDIR /app

COPY . .

ENV RUSTFLAGS="-C target-feature=-crt-static"
RUN sed -i -e "s/^version = .*/version = \"${BUILD_VERSION}\"/" Cargo.toml
RUN cargo install --path .

FROM alpine:3.20

ARG BUILD_VERSION=0.0.0-development
ARG COMMIT_SHA=NOT_AVAILABLE

LABEL org.opencontainers.image.authors="cbuehler@rootd.ch" \
    org.opencontainers.image.url="https://github.com/WirePact/k8s-contract-provider" \
    org.opencontainers.image.documentation="https://github.com/WirePact/k8s-contract-provider/blob/main/README.md" \
    org.opencontainers.image.source="https://github.com/WirePact/k8s-contract-provider/blob/main/Dockerfile" \
    org.opencontainers.image.version="${BUILD_VERSION}" \
    org.opencontainers.image.revision="${COMMIT_SHA}" \
    org.opencontainers.image.licenses="Apache-2.0" \
    org.opencontainers.image.title="WirePact Contract Provider" \
    org.opencontainers.image.description="Module for WirePact that continuously fetches all valid contracts for its own trust zone and stores them in a local file or a Kubernetes secret."

WORKDIR /app

ENV USER=appuser \
    UID=1000 \
    BUILD_VERSION=${BUILD_VERSION} \
    FETCH_INTERVAL=5min

COPY --from=build /usr/local/cargo/bin/k8s-contract-provider /usr/local/bin/k8s-contract-provider

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}" && \
    chown -R appuser:appuser /app && \
    chmod +x /usr/local/bin/k8s-contract-provider && \
    apk add --update --no-cache libgcc ca-certificates

USER appuser:appuser

ENTRYPOINT ["/usr/local/bin/k8s-contract-provider"]
