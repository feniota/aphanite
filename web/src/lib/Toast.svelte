<script lang="ts">
  import { getToasts, dismiss, pauseTimer, resumeTimer } from "./toast.svelte";
  const toasts = $derived(getToasts());
</script>

{#if toasts.length > 0}
  <div class="fixed top-4 right-4 z-100 flex flex-col gap-3">
    {#each toasts as t (t.id)}
      <div
        class="flex min-w-72 items-center gap-3 rounded-xl border border-slate-200 bg-white px-5 py-3.5 text-sm shadow-lg dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200"
        onmouseenter={() => pauseTimer(t.id)}
        onmouseleave={() => resumeTimer(t.id)}>
        <span class="flex-1">{t.message}</span>
        <button
          onclick={() => dismiss(t.id)}
          class="cursor-pointer rounded p-0.5 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300">
          <svg
            class="h-4 w-4"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2">
            <path d="M18 6L6 18M6 6l12 12" stroke-linecap="round" />
          </svg>
        </button>
      </div>
    {/each}
  </div>
{/if}
