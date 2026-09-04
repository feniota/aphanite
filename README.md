# Aphanite

[![Ferris.love badge](https://ferris.love/badge/feniota/aphanite?variant=mini)](https://ferris.love/feniota/aphanite)

An open-source, self-deployable, high-performance [Yggdrasil](https://github.com/yushijinhun/authlib-injector) server.

## Website

See [offical Wiki](https://phenocryst.ferris.love/aphanite/).

## Container

A docker image is available as `quay.io/feniota/aphanite`. See [the Wiki page](https://phenocryst.ferris.love/aphanite/installation#docker) for usage instructions. See [Dockerfile](./Dockerfile) for details.

## Development

You'll need [Rust toolchain](https://rustup.rs) and [Deno](https://deno.com/) to develop Aphanite.

```bash
# Install NPM dependencies
deno install

# Start Aphanite in development mode
deno task dev

# Build Aphanite for a single OS
deno run -A ./scripts/build.ts x86_64-unknown-linux-gnu
```
