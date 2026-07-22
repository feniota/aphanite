<script lang="ts">
  import { X } from "@lucide/svelte";
  import { fade } from "svelte/transition";

  import { get_toasts, dismiss, pause_timer, resume_timer } from "./toast.svelte";
  const toasts = $derived(get_toasts());
</script>

{#if toasts.length > 0}
  <div class="fixed right-4 bottom-8 z-100 flex flex-col gap-4">
    {#each toasts as t (t.id)}
      <div
        transition:fade|global={{ duration: 300 }}
        class="bg-surface flex min-w-90 items-center gap-3 rounded-xl border px-5 py-3.5 text-sm"
        onmouseenter={() => pause_timer(t.id)}
        onmouseleave={() => resume_timer(t.id)}>
        <span class="flex-1">{t.message}</span>
        <button
          type="button"
          onclick={() => dismiss(t.id)}
          class="text-muted-foreground hover:text-primary-foreground cursor-pointer p-0.5">
          <X />
        </button>
      </div>
    {/each}
  </div>
{/if}
