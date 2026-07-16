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

  $effect(() => {
    if (!auth.isLoggedIn) {
      window.location.href = "/login";
    }
  });
</script>

{#if auth.isLoggedIn}
  <Toast />
  <div class="flex h-screen bg-slate-50 dark:bg-slate-900">
    <Sidebar />
    <main class="flex-1 overflow-auto">
      <Router {routes} />
    </main>
  </div>
{/if}
