# `capes.json` README

This file would be synced to `https://assets.ferris.love/phenocryst/capes/capes.json`, which is a Cloudflare R2 public bucket, and its schema is located at `https://phenocryst.ferris.love/aphanite-cape-list-schema.json`. (source [here](https://github.com/feniota/phenocryst-docs/blob/main/public/aphanite-cape-list-schema.json))

The capes listed in that files are all copyright Mojang Studios. The goal we share them on assets.ferris.love is to allow players using Aphanite to use them in Minecraft game.

## FAQ

### Where is it used?

This file, and the capes it listed, are fetched and shown when a user attempts to upload their capes. (as written in [PlayerProfileDetails.svelte](../pages/PlayerProfileDetails.svelte))

### Why on a separate site?

To dynamically update the available capes without updating Aphanite's source code. Also, putting these default assets on a managed, [fast](https://www.cloudflare.com/application-services/use-cases/performance/), widely available, and egress-fee-free platform can save Aphanite deployers' traffic usage, since not all capes shown there would be actually uploaded to your server.

### They are all official Minecraft assets, why not just reference textures.minecraft.net?

To prevent CORS issues.

### I want to contribute.

You can also freely contribute to this file and [the schema](https://github.com/feniota/phenocryst-docs/blob/main/public/aphanite-cape-list-chema.json). We will make sure to sync it to the Cloud once your modifications are merged.
