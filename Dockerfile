FROM docker.io/denoland/deno:debian AS frontend

WORKDIR /src

COPY package.json deno.jsonc deno.lock ./
COPY web ./web

RUN deno install --frozen \
    && deno x vite build web/

FROM docker.io/library/rust:1-slim-trixie AS builder

WORKDIR /src

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY build ./build
COPY migrations ./migrations
COPY --from=frontend /src/web/dist ./web/dist

RUN cargo build --release

FROM quay.io/fedora/fedora-minimal:45

WORKDIR /app

RUN useradd --system --create-home --home-dir /app --shell /sbin/nologin aphanite \
    && mkdir -p /app/data \
    && chown -R aphanite:aphanite /app

COPY --from=builder --chown=aphanite:aphanite /src/target/release/aphanite /usr/local/bin/aphanite
COPY scripts/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh

RUN chmod +x /usr/local/bin/docker-entrypoint.sh

ENV RUST_LOG=aphanite=info

EXPOSE 3000
VOLUME ["/app/data"]
USER aphanite

CMD ["--listen", "0.0.0.0"]
ENTRYPOINT ["/bin/sh", "/usr/local/bin/docker-entrypoint.sh"]
