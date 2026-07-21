<script lang="ts">
  import { ArrowLeft } from "@lucide/svelte";
  import { link } from "svelte-spa-router";
  import active from "svelte-spa-router/active";

  import DarkModeButton from "@/components/DarkModeButton.svelte";
  import { SIDEBAR } from "@/lib/sidebar.svelte";
  import { cn } from "@/lib/utils";

  import { routes_with_title } from "./home_page_router.svelte";

  const { class: class_name }: { class: string } = $props();
</script>

<!-- Backdrop -->
<div
  class="backdrop bg-background/40 fixed top-0 left-0 z-40 min-h-dvh min-w-dvw transition-[backdrop-filter,opacity] duration-300 ease-in-out lg:hidden!"
  class:open={SIDEBAR.open}
  class:pointer-events-none={!SIDEBAR.open}
  class:opacity-0={!SIDEBAR.open}
  class:opacity-100={SIDEBAR.open}
  onclick={() => (SIDEBAR.open = false)}
  onkeydown={e => e.key === "Enter" && (SIDEBAR.open = false)}
  role="button"
  tabindex="-1">
  <!-- Collapse button -->
  <div
    class="bg-background hover:bg-surface text-primary-foreground fixed top-[45dvh] left-[80vw] flex h-[10dvh] w-8 cursor-pointer items-center justify-center rounded-r-xl border-t border-r border-b sm:left-[60vw] md:left-[40vw]"
    onclick={e => {
      e.stopPropagation();
      SIDEBAR.open = false;
    }}>
    <ArrowLeft />
  </div>
</div>

<!-- Sidebar panel -->
<div
  class={cn(
    "bg-background fixed top-0 left-0 z-50 flex min-h-dvh min-w-[80vw] flex-col divide-y border-r pt-15 transition-transform duration-300 ease-in-out sm:min-w-[60vw] md:min-w-[40vw] lg:sticky lg:top-15 lg:z-auto lg:h-[calc(100dvh-var(--spacing)*15)] lg:min-h-0 lg:min-w-0 lg:translate-x-0 lg:pt-0",
    class_name,
  )}
  class:-translate-x-full={!SIDEBAR.open}
  class:translate-x-0={SIDEBAR.open}>
  {#each routes_with_title as item}
    <a
      use:link
      use:active
      class="hover:from-surface [&.active]:from-surface/70 [&.active]:hover:from-surface block bg-linear-to-l from-transparent to-transparent py-4 pl-4 font-light first:border-t last:border-b focus:ring-0 focus:outline-none lg:py-2 first:lg:border-t-0"
      href={item.path}>{item.title}</a>
  {/each}
  <div class="flex-1 border-b"></div>
  <div class="flex h-15 flex-row items-center px-4">
    <DarkModeButton />
  </div>
</div>

<style>
  .backdrop {
    --aph-backdrop-blur: 0px;
    backdrop-filter: blur(var(--aph-backdrop-blur));
  }
  .backdrop.open {
    --aph-backdrop-blur: 12px;
  }
  @media (min-width: 48rem) {
    .backdrop.open {
      --aph-backdrop-blur: 0px;
    }
  }
</style>
