# Aphanite

[![Ferris.love badge](https://ferris.love/badge/feniota/aphanite?variant=mini)](https://ferris.love/feniota/aphanite) [![CI status for backend build](https://git.feniota.org/feniota/aphanite/badges/workflows/rust.yml/badge.svg)](https://git.feniota.org/feniota/aphanite/actions?workflow=rust.yml) [![CI status for frontend build](https://git.feniota.org/feniota/aphanite/badges/workflows/frontend.yml/badge.svg)](https://git.feniota.org/feniota/aphanite/actions?workflow=frontend.yml) [![Latest Release](https://git.feniota.org/feniota/aphanite/badges/release.svg)](https://git.feniota.org/feniota/aphanite/releases/latest)

An open-source, self-deployable, high-performance [Yggdrasil](https://github.com/yushijinhun/authlib-injector) server.

## Website

See [offical Wiki](https://mc.feniota.org/aphanite/).

## Container

A docker image is available as `quay.io/feniota/aphanite` or `ghcr.io/feniota/aphanite`. See [the Wiki page](https://mc.feniota.org/aphanite/installation#docker) for usage instructions. See [Dockerfile](./Dockerfile) for details.

## Development

You'll need the [Rust toolchain](https://rustup.rs), [Node.js](https://nodejs.org/) (v24+) and [pnpm](https://pnpm.io/) to develop Aphanite.

```bash
# Install dependencies
pnpm install

# Start Aphanite in development mode
pnpm dev

# Build Aphanite for a single OS
pnpm build x86_64-unknown-linux-gnu
```
