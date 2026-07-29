<script lang="ts">
  import XIcon from "@lucide/svelte/icons/x";
  import { fade } from "svelte/transition";

  import * as BDialog from "@/lib/components/ui/dialog";
  import { cn } from "@/lib/utils";

  let _open = $state(false);

  let {
    children,
    class: className,
    close_on_esc = true,
    close_on_backdrop = true,
    disable_close_btn = false,
    no_default_styles = false,
    onclose,
  }: {
    children: import("svelte").Snippet;
    class?: string;
    close_on_esc?: boolean;
    close_on_backdrop?: boolean;
    disable_close_btn?: boolean;
    no_default_styles?: boolean;
    onclose?: () => void;
  } = $props();

  export function open() {
    _open = true;
  }

  export function close() {
    _open = false;
    onclose?.();
  }

  function handle_backdrop_click(e: MouseEvent) {
    if (close_on_backdrop && e.target === e.currentTarget) close();
  }
</script>

{#if _open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    role="presentation"
    class="--aph-backdrop fixed inset-0 z-40"
    style="background-color: color-mix(in oklab, var(--color-background) 65%, transparent)"
    onclick={handle_backdrop_click}
    transition:fade={{ duration: 200 }}>
  </div>
  <div
    class="--aph-dialog-wrapper pointer-events-none fixed inset-0 z-50 grid place-items-center"
    transition:fade={{ duration: 150 }}>
    <BDialog.Root
      open={_open}
      onOpenChange={v => {
        if (!v) close();
      }}>
      <BDialog.Content
        showCloseButton={false}
        portalProps={{ portal: false }}
        class={cn(
          "pointer-events-auto w-120 max-w-[calc(100%-2rem)] rounded-xl! shadow-none ring-0",
          !no_default_styles && "bg-background text-foreground border-border border p-4",
          className,
        )}
        interactOutsideBehavior={close_on_backdrop ? "close" : "ignore"}
        escapeKeydownBehavior={close_on_esc ? "close" : "ignore"}
        onOpenAutoFocus={e => e.preventDefault()}
        onCloseAutoFocus={e => e.preventDefault()}>
        {#if no_default_styles}
          {@render children?.()}
        {:else}
          <div class="flex flex-col gap-2">
            {@render children?.()}
            {#if !disable_close_btn}
              <BDialog.Close
                class="--aph-dialog-button hover:bg-surface absolute top-4 right-4 rounded p-0.5">
                <XIcon class="size-5" />
              </BDialog.Close>
            {/if}
          </div>
        {/if}
      </BDialog.Content>
    </BDialog.Root>
  </div>
{/if}

<style>
  .--aph-backdrop {
    backdrop-filter: blur(12px);
    transition: backdrop-filter 0.25s ease-in-out;
  }
  @media (min-width: 48rem) {
    .--aph-backdrop {
      backdrop-filter: blur(0px);
    }
  }

  :global([data-slot="dialog-overlay"]) {
    display: none !important;
  }
</style>
