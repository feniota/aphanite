<script lang="ts">
  import Router from "svelte-spa-router";

  import { AUTH } from "./lib/auth.svelte";
  import Sidebar from "./lib/Sidebar.svelte";
  import Toast from "./lib/Toast.svelte";
  import Dashboard from "./pages/Dashboard.svelte";
  import Profile from "./pages/Profile.svelte";
  import Profiles from "./pages/Profiles.svelte";
  import Users from "./pages/Users.svelte";

  const routes = {
    "/": Dashboard,
    "/profiles": Profiles,
    "/my": Profile,
    "/users": Users,
  };

  let sidebar_open = $state(false);

  $effect(() => {
    if (!AUTH.is_logged_in) {
      window.location.href = "/login";
    }
  });
</script>

{#if AUTH.is_logged_in}
  <Toast />
  <div class="flex h-screen bg-slate-50 dark:bg-slate-900">
    <Sidebar bind:open={sidebar_open} />
    <main class="flex-1 overflow-auto">
      <button
        class="sticky top-0 z-20 flex h-12 items-center border-b border-slate-200 bg-slate-50 px-4 md:hidden dark:border-slate-700 dark:bg-slate-900"
        onclick={() => (sidebar_open = !sidebar_open)}>
        {#if sidebar_open}
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
