<script lang="ts">
  import { X } from "@lucide/svelte";

  import { cn } from "@/lib/utils";

  let dialog_elem: HTMLDialogElement | null = $state(null);

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
    /**
     * Whether not to put a default close button (the X at the top-right corner) in the dialog.
     */
    disable_close_btn?: boolean;
    /**
     * Whether not to specify the default styles for the dialog surface. Backdrop is not affected by this prop.
     * This also disables the close button.
     * */
    no_default_styles?: boolean;
    onclose?: () => void;
  } = $props();

  export function open() {
    dialog_elem?.showModal();
  }

  export function close() {
    dialog_elem?.close();
  }

  function handle_click(e: MouseEvent) {
    if (!close_on_backdrop) return;
    if (!dialog_elem) return;
    const rect = dialog_elem.getBoundingClientRect();
    const clicked_outside =
      e.clientX < rect.left ||
      e.clientX > rect.right ||
      e.clientY < rect.top ||
      e.clientY > rect.bottom;
    if (clicked_outside) {
      close();
    }
  }

  function handle_keydown(e: KeyboardEvent) {
    if (e.key === "Escape" && !close_on_esc) {
      e.preventDefault();
    }
  }
</script>

<dialog
  bind:this={dialog_elem}
  class={cn(
    "--aph-dialog",
    !no_default_styles &&
      "text-foreground bg-background border-border relative inset-0 m-auto w-120 rounded-xl border p-4 focus:ring-0",
    className,
  )}
  onclick={handle_click}
  onkeydown={handle_keydown}
  {onclose}
  role="dialog"
  aria-modal="true">
  {#if no_default_styles}
    {@render children?.()}
  {:else}
    <div class="flex flex-col gap-2">
      {@render children?.()}
      {#if !disable_close_btn}
        <!-- Last element to prevent it from stealing the focus -->
        <button
          class="--aph-dialog-button hover:bg-surface absolute top-4 right-4 rounded p-0.5"
          type="button"
          onclick={close}><X /></button>
      {/if}
    </div>
  {/if}
</dialog>

<style>
  dialog {
    opacity: 0;
    transition:
      opacity 0.2s ease-in-out,
      display 0.2s ease-in-out allow-discrete,
      overlay 0.2s ease-in-out allow-discrete;
  }

  dialog[open] {
    opacity: 1;
  }

  @starting-style {
    dialog[open] {
      opacity: 1;
    }
  }

  dialog::backdrop {
    background-color: color-mix(in oklab, var(--color-background) 60%, transparent);
    backdrop-filter: blur(0px);
    transition:
      backdrop-filter 0.3s ease-in-out,
      opacity 0.3s ease-in-out;
    opacity: 0;
  }

  dialog[open]::backdrop {
    backdrop-filter: blur(12px);
    opacity: 1;
  }

  @starting-style {
    dialog[open]::backdrop {
      backdrop-filter: blur(0px);
      opacity: 0;
    }
  }

  @media (min-width: 48rem) {
    dialog[open]::backdrop {
      backdrop-filter: blur(0px);
    }
  }

  .--aph-dialog {
    view-transition-name: --aph-dialog;
  }
  .--aph-dialog-button {
    view-transition-name: --aph-dialog-button;
  }
</style>
