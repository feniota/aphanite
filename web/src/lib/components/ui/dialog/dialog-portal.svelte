<script lang="ts">
  import { Dialog as DialogPrimitive } from "bits-ui";
  import type { Snippet } from "svelte";

  let {
    portal = true,
    children,
    ...restProps
  }: DialogPrimitive.PortalProps & { portal?: boolean; children?: Snippet } = $props();
</script>

{#if portal}
  <DialogPrimitive.Portal {...restProps}>
    {@render children?.()}
  </DialogPrimitive.Portal>
{:else}
  {@render children?.()}
{/if}
