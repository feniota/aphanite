FROM quay.io/lingrottin/runners:rust-musl AS build
WORKDIR /src

COPY package.json pnpm-lock.yaml .npmrc ./
RUN pnpm install --frozen-lockfile

COPY Cargo.toml Cargo.lock ./
COPY build ./build
COPY migrations ./migrations
COPY src ./src
COPY web ./web

RUN pnpm exec vite build web/ \
 && cargo build --locked --release --features bundle-certs --target=x86_64-unknown-linux-musl

RUN mkdir -p /out/app-stage/app/data /out/tmp-stage/tmp \
 && chown 1000:1000 /out/app-stage/app /out/app-stage/app/data \
 && chmod 1777 /out/tmp-stage/tmp

FROM scratch

# copy staging roots' contents so directory modes survive
COPY --from=build --chown=1000:1000 /out/app-stage/ /
# turso's tempfile usage needs a writable temp dir
COPY --from=build --chown=0:0 /out/tmp-stage/ /
COPY --from=build /src/target/x86_64-unknown-linux-musl/release/aphanite /usr/local/bin/aphanite

ENV APHANITE_AUTO_INIT=1 \
    RUST_LOG=aphanite=info \
    TMPDIR=/tmp

WORKDIR /app
EXPOSE 3000
USER 1000:1000

ENTRYPOINT ["/usr/local/bin/aphanite"]
CMD ["--listen", "0.0.0.0"]
