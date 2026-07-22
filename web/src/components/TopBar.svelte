<script lang="ts">
  import { Menu } from "@lucide/svelte";
  import { router } from "svelte-spa-router";

  import { routes_with_title } from "@/lib/home-page-router";
  import { SIDEBAR } from "@/lib/sidebar.svelte";
  const current_title = $derived(
    routes_with_title.find(x => x.path === router.location)?.title ?? "首页",
  );
</script>

<header class="bg-background fixed top-0 z-2 flex w-full flex-col">
  <div class="flex h-15 w-full flex-row items-center justify-between border-b px-5">
    <div class="flex flex-1 flex-row items-center lg:hidden">
      <button
        type="button"
        class="text-muted-foreground hover:bg-surface mr-3 cursor-pointer rounded-sm p-1 transition-colors duration-200 focus:ring"
        onclick={() => (SIDEBAR.open = true)}>
        <Menu class="size-5"></Menu>
      </button> <span class="text-primary-foreground text-lg font-extralight">{current_title}</span>
    </div>
    <span class="text-primary-foreground hidden flex-1 text-lg font-extralight lg:block"
      >Aphanite Dashboard</span>
    <div class="flex flex-1 flex-row justify-end">foobar</div>
  </div>
</header>
