<script lang="ts">
  import { listUsers, createUser, ApiError } from "../lib/api";
  import type { User, Permission, CreateUserResponse } from "../lib/api";
  import { auth } from "../lib/auth.svelte";

  let users = $state<User[]>([]);
  let loading = $state(true);
  let error = $state("");

  let createModal = $state(false);
  let newEmail = $state("");
  let newName = $state("");
  let newManagement = $state(false);
  let createLoading = $state(false);
  let createError = $state("");
  let createdUser = $state<CreateUserResponse | null>(null);
  let pwdCopied = $state(false);

  async function load() {
    if (!auth.token) return;
    loading = true;
    error = "";
    try {
      users = await listUsers(auth.token);
    } catch (err) {
      error = err instanceof ApiError ? "加载用户列表失败" : "网络错误";
    } finally {
      loading = false;
    }
  }

  function openCreate() {
    newEmail = "";
    newName = "";
    newManagement = false;
    createError = "";
    createdUser = null;
    pwdCopied = false;
    createModal = true;
  }

  function copyPassword() {
    if (!createdUser) return;
    navigator.clipboard.writeText(createdUser.password);
    pwdCopied = true;
    setTimeout(() => (pwdCopied = false), 2000);
  }

  async function submitCreate(e: SubmitEvent) {
    e.preventDefault();
    if (!auth.token) return;
    createError = "";
    createLoading = true;
    try {
      const perms: Permission[] = newManagement ? ["management"] : [];
      const result = await createUser(auth.token, {
        email: newEmail,
        name: newName || undefined,
        permissions: perms,
      });
      createdUser = result;
      load();
    } catch (err) {
      if (err instanceof ApiError) {
        createError =
          err.status === 409
            ? "该邮箱已存在"
            : err.status === 422
              ? "昵称或邮箱格式不正确"
              : "创建失败，请重试";
      } else {
        createError = "网络错误，请检查网络连接";
      }
    } finally {
      createLoading = false;
    }
  }

  $effect(() => {
    load();
  });
</script>

<div class="p-8">
  <div class="mx-auto max-w-4xl">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold text-slate-900 dark:text-slate-100">用户管理</h1>
        <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">管理系统用户及其权限</p>
      </div>
      <button
        onclick={openCreate}
        class="cursor-pointer rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-700 dark:bg-indigo-500 dark:hover:bg-indigo-600">
        创建用户
      </button>
    </div>

    {#if loading}
      <p class="mt-8 text-center text-sm text-slate-400">加载中…</p>
    {:else if error}
      <p class="mt-8 text-center text-sm text-slate-500 dark:text-slate-400">{error}</p>
    {:else}
      <div
        class="mt-6 overflow-hidden rounded-xl border border-slate-200 bg-white dark:border-slate-700 dark:bg-slate-800">
        <table class="w-full text-sm">
          <thead
            class="border-b border-slate-200 bg-slate-50 dark:border-slate-700 dark:bg-slate-900">
            <tr>
              <th class="px-4 py-3 text-left font-medium text-slate-500 dark:text-slate-400"
                >邮箱</th>
              <th class="px-4 py-3 text-left font-medium text-slate-500 dark:text-slate-400"
                >昵称</th>
              <th class="px-4 py-3 text-left font-medium text-slate-500 dark:text-slate-400"
                >权限</th>
            </tr>
          </thead>
          <tbody>
            {#each users as u (u.id)}
              <tr class="border-b border-slate-100 last:border-0 dark:border-slate-700">
                <td class="px-4 py-3 text-slate-900 dark:text-slate-100">{u.email}</td>
                <td class="px-4 py-3 text-slate-700 dark:text-slate-300">{u.name}</td>
                <td class="px-4 py-3">
                  {#if u.permissions.includes("management")}
                    <span
                      class="rounded-full bg-indigo-100 px-2 py-0.5 text-xs font-medium text-indigo-700 dark:bg-indigo-900 dark:text-indigo-300"
                      >管理员</span>
                  {:else}
                    <span class="text-xs text-slate-400 dark:text-slate-500">—</span>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}

    {#if createModal}
      <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/30"
        onclick={() => (createModal = false)}
        onkeydown={(e) => e.key === "Escape" && (createModal = false)}
        role="presentation">
        <div
          class="w-full max-w-lg rounded-2xl bg-white p-6 shadow-xl dark:bg-slate-800"
          onclick={(e) => e.stopPropagation()}
          onkeydown={() => {}}
          role="presentation">
          {#if createdUser}
            <h3 class="text-lg font-semibold text-slate-900 dark:text-slate-100">用户已创建</h3>
            <div class="mt-4 space-y-2 rounded-lg bg-slate-50 p-4 dark:bg-slate-900">
              <div class="flex gap-2 text-sm">
                <span class="shrink-0 text-slate-500 dark:text-slate-400">邮箱：</span>
                <span class="font-mono text-slate-900 dark:text-slate-100"
                  >{createdUser.email}</span>
              </div>
              <div class="flex items-start gap-2 text-sm">
                <span class="shrink-0 text-slate-500 dark:text-slate-400">初始密码：</span>
                <span class="font-mono break-all text-indigo-600 select-all dark:text-indigo-400"
                  >{createdUser.password}</span>
                <button
                  onclick={copyPassword}
                  class="shrink-0 cursor-pointer rounded p-0.5 text-slate-400 hover:text-indigo-600 dark:hover:text-indigo-400">
                  {#if pwdCopied}
                    <svg
                      class="h-4 w-4 text-emerald-500"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2">
                      <path d="M20 6L9 17l-5-5" stroke-linecap="round" stroke-linejoin="round" />
                    </svg>
                  {:else}
                    <svg
                      class="h-4 w-4"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2">
                      <rect x="9" y="9" width="13" height="13" rx="2" />
                      <path
                        d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"
                        stroke-linecap="round" />
                    </svg>
                  {/if}
                </button>
              </div>
            </div>
            <p class="mt-3 text-xs text-amber-600 dark:text-amber-400">
              请妥善保管初始密码，此信息不会再次显示
            </p>
            <div class="mt-6 text-right">
              <button
                onclick={() => (createModal = false)}
                class="cursor-pointer rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-700 dark:bg-indigo-500 dark:hover:bg-indigo-600">
                完成
              </button>
            </div>
          {:else}
            <h3 class="text-lg font-semibold text-slate-900 dark:text-slate-100">创建用户</h3>
            <form class="mt-4 space-y-4" onsubmit={submitCreate}>
              <div>
                <label
                  for="create-email"
                  class="block text-sm font-medium text-slate-700 dark:text-slate-300">邮箱</label>
                <input
                  id="create-email"
                  type="email"
                  bind:value={newEmail}
                  required
                  placeholder="user@example.com"
                  class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100" />
              </div>
              <div>
                <label
                  for="create-name"
                  class="block text-sm font-medium text-slate-700 dark:text-slate-300">昵称</label>
                <input
                  id="create-name"
                  type="text"
                  bind:value={newName}
                  placeholder="留空则使用邮箱"
                  class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100" />
              </div>
              <div class="flex justify-end">
                <label class="flex items-center gap-2 text-sm text-slate-700 dark:text-slate-300">
                  <input
                    type="checkbox"
                    bind:checked={newManagement}
                    class="h-4 w-4 accent-indigo-600" />
                  管理员权限
                </label>
              </div>
              {#if createError}
                <p class="text-sm text-red-600 dark:text-red-400">{createError}</p>
              {/if}
              <div class="flex justify-end gap-2">
                <button
                  type="button"
                  onclick={() => (createModal = false)}
                  class="cursor-pointer rounded-lg border border-slate-300 px-4 py-2 text-sm text-slate-600 dark:border-slate-600 dark:text-slate-400">
                  取消
                </button>
                <button
                  type="submit"
                  disabled={createLoading || !newEmail}
                  class="cursor-pointer rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-700 disabled:opacity-60 dark:bg-indigo-500 dark:hover:bg-indigo-600">
                  {createLoading ? "创建中…" : "创建"}
                </button>
              </div>
            </form>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>
