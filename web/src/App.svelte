<script lang="ts">
  import Router from "svelte-spa-router";

  import { auth } from "./lib/auth.svelte";
  import Sidebar from "./lib/Sidebar.svelte";
  import Toast from "./lib/Toast.svelte";
  import Dashboard from "./pages/Dashboard.svelte";
  import Profile from "./pages/Profile.svelte";
  import Profiles from "./pages/Profiles.svelte";

  const routes = {
    "/": Dashboard,
    "/profiles": Profiles,
    "/profile": Profile,
  };

  let sidebarOpen = $state(false);

  $effect(() => {
    if (!auth.isLoggedIn) {
      window.location.href = "/login";
    }
  });
</script>

{#if auth.isLoggedIn}
  <Toast />
  <div class="flex h-screen bg-slate-50 dark:bg-slate-900">
    <Sidebar bind:open={sidebarOpen} />
    <main class="flex-1 overflow-auto">
      <button
        class="sticky top-0 z-20 flex h-12 items-center border-b border-slate-200 bg-slate-50 px-4 md:hidden dark:border-slate-700 dark:bg-slate-900"
        onclick={() => (sidebarOpen = !sidebarOpen)}>
        {#if sidebarOpen}
          <!-- X -->
          <svg
            class="h-5 w-5 text-slate-600 dark:text-slate-400"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2">
            <path d="M6 6l12 12M18 6L6 18" stroke-linecap="round" />
          </svg>
        {:else}
          <!-- 汉堡 -->
          <svg
            class="h-5 w-5 text-slate-600 dark:text-slate-400"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2">
            <path d="M3 6h18M3 12h18M3 18h18" stroke-linecap="round" />
          </svg>
        {/if}
      </button>
      <Router {routes} />
    </main>
  </div>
{/if}
