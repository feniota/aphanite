<script lang="ts">
  import { push } from "svelte-spa-router";

  import { AUTH } from "./auth.svelte";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  const nav = [
    { path: "/", label: "仪表板" },
    { path: "/profiles", label: "角色管理" },
    { path: "/profile", label: "个人资料" },
    ...(AUTH.user?.permissions.includes("management")
      ? [{ path: "/users", label: "用户管理" }]
      : []),
  ];

  let current = $state("/");

  function get_hash_path() {
    return window.location.hash.slice(1) || "/";
  }

  $effect(() => {
    current = get_hash_path();
    const onHash = () => (current = get_hash_path());
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  });

  function navigate(path: string) {
    current = path;
    push(path);
    open = false;
  }
</script>

{#if open}
  <!-- 移动端遮罩 -->
  <div
    class="fixed inset-0 z-30 bg-black/30 md:hidden"
    onclick={() => (open = false)}
    onkeydown={e => e.key === "Escape" && (open = false)}
    role="presentation">
  </div>
{/if}

<aside
  class="fixed inset-y-0 left-0 z-40 flex w-56 flex-col border-r border-slate-200 bg-white transition-transform md:static md:translate-x-0 dark:border-slate-700 dark:bg-slate-800"
  class:-translate-x-full={!open}>
  <div class="flex h-14 items-center border-b border-slate-200 px-4 dark:border-slate-700">
    <span class="text-lg font-bold text-indigo-600 dark:text-indigo-400">aphanite</span>
  </div>

  <nav class="flex-1 space-y-1 p-3">
    {#each nav as item}
      <button
        onclick={() => navigate(item.path)}
        class="block w-full rounded-lg px-3 py-2 text-left text-sm transition {current === item.path
          ? 'bg-indigo-50 text-indigo-700 dark:bg-indigo-950 dark:text-indigo-300'
          : 'text-slate-600 hover:bg-slate-100 dark:text-slate-400 dark:hover:bg-slate-700'}">
        {item.label}
      </button>
    {/each}
  </nav>

  <div class="border-t border-slate-200 p-3 dark:border-slate-700">
    <button
      onclick={() => {
        AUTH.logout();
        window.location.href = "/login";
      }}
      class="block w-full rounded-lg px-3 py-2 text-left text-sm text-slate-500 hover:bg-slate-100 dark:text-slate-400 dark:hover:bg-slate-700">
      退出登录
    </button>
    <p class="mt-1 truncate px-3 text-xs text-slate-400 dark:text-slate-500">
      {AUTH.user?.email ?? ""}
    </p>
  </div>
</aside>
