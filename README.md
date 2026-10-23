# Aphanite

[![Ferris.love badge](https://ferris.love/badge/feniota/aphanite?variant=mini)](https://ferris.love/feniota/aphanite)

An open-source, self-deployable, high-performance [Yggdrasil](https://github.com/yushijinhun/authlib-injector) server.

## Website

See [offical Wiki](https://mc.feniota.org/aphanite/).

## Container

A docker image is available as `quay.io/feniota/aphanite`. See [the Wiki page](https://mc.feniota.org/aphanite/installation#docker) for usage instructions. See [Dockerfile](./Dockerfile) for details.

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
