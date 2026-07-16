<script lang="ts">
  import { register, getTurnstileSiteKey, ApiError } from "../lib/api";
  import AuthImage from "../lib/AuthImage.svelte";

  // "loading" | "public" | "public_turnstile" | "private"
  let mode = $state("loading");
  let siteKey = $state<string | null>(null);
  let registerToken = $state<string | undefined>(undefined);
  let turnstileEl = $state<HTMLDivElement | null>(null);
  let turnstileId = $state("");

  let email = $state("");
  let name = $state("");
  let password = $state("");
  let confirm = $state("");
  let loading = $state(false);
  let error = $state("");
  let success = $state(false);

  $effect(() => {
    const p = new URLSearchParams(window.location.search);
    registerToken = p.get("token") || undefined;

    getTurnstileSiteKey()
      .then(({ site_key }) => {
        siteKey = site_key;
        mode = "public_turnstile";
      })
      .catch((err) => {
        if (err instanceof ApiError) {
          console.log("[register] Turnstile API status:", err.status, err.message);
          mode = err.status === 404 ? "public" : "private";
        } else {
          console.log("[register] Turnstile API network error:", err);
          mode = "error";
        }
      });
  });

  $effect(() => {
    if (!siteKey || !turnstileEl) return;

    const ts = (window as any).turnstile;
    const existing = document.querySelector('script[src*="turnstile"]');

    const render = () => {
      const id = ts?.render(turnstileEl!, {
        sitekey: siteKey,
        theme: window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light",
      });
      if (id) turnstileId = id;
    };

    if (existing) {
      if (ts) {
        render();
      } else {
        existing.addEventListener("load", render, { once: true });
      }
    } else {
      const s = document.createElement("script");
      s.src = "https://challenges.cloudflare.com/turnstile/v0/api.js";
      s.async = true;
      s.defer = true;
      s.onload = render;
      document.head.appendChild(s);
    }

    return () => {
      if (turnstileId) ts?.remove(turnstileId);
    };
  });

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    error = "";

    if (password !== confirm) {
      error = "两次输入的密码不一致";
      return;
    }

    loading = true;
    try {
      const ts = (window as any).turnstile;
      await register({
        email,
        name: name || undefined,
        password,
        turnstile_token: turnstileId ? ts?.getResponse(turnstileId) : undefined,
        register_token: registerToken,
      });
      success = true;
    } catch (err) {
      error = err instanceof ApiError ? err.message : "网络错误，请检查后端是否已启动";
      if (turnstileId) (window as any).turnstile?.reset(turnstileId);
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex min-h-screen">
  <div
    class="hidden bg-indigo-600 md:flex md:w-[70%] md:items-center md:justify-center md:p-16 dark:bg-indigo-950">
    <AuthImage />
  </div>

  <div class="flex w-full items-center justify-center bg-slate-50 p-8 md:w-[30%] dark:bg-slate-900">
    <div class="w-full max-w-sm space-y-6">
      <div class="text-center">
        <h1 class="text-3xl font-bold text-slate-900 dark:text-slate-100">注册</h1>
        <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
          {mode === "loading" ? "…" : "创建你的 aphanite 账号"}
        </p>
      </div>

      {#if mode === "loading"}
        <p class="text-center text-sm text-slate-400">加载中…</p>
      {:else if mode === "private" && !registerToken}
        <div class="text-center">
          <p class="text-sm leading-relaxed text-slate-600 dark:text-slate-400">
            当前服务器未开放公开注册<br />请联系管理员获取邀请链接
          </p>
          <a
            href="#/"
            class="mt-4 inline-block text-sm font-medium text-indigo-600 hover:text-indigo-800 dark:text-indigo-400 dark:hover:text-indigo-300"
            >← 返回登录</a>
        </div>
      {:else if mode === "error"}
        <div class="text-center">
          <p class="text-sm leading-relaxed text-slate-600 dark:text-slate-400">
            无法连接服务器<br />请检查后端是否已启动
          </p>
          <a
            href="#/"
            class="mt-4 inline-block text-sm font-medium text-indigo-600 hover:text-indigo-800 dark:text-indigo-400 dark:hover:text-indigo-300"
            >← 返回登录</a>
        </div>
      {:else if success}
        <div class="text-center">
          <p class="text-sm leading-relaxed text-slate-600 dark:text-slate-400">
            注册成功！<a
              href="#/"
              class="font-medium text-indigo-600 hover:text-indigo-800 dark:text-indigo-400 dark:hover:text-indigo-300"
              >去登录</a>
          </p>
        </div>
      {:else}
        {#if error}
          <div
            class="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-800 dark:bg-red-950 dark:text-red-400">
            {error}
          </div>
        {/if}

        <form class="space-y-4" onsubmit={handleSubmit}>
          <div>
            <label
              for="reg-username"
              class="block text-sm font-medium text-slate-700 dark:text-slate-300">用户名</label>
            <input
              id="reg-username"
              type="text"
              bind:value={name}
              placeholder="User 玩家名"
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm transition outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100" />
          </div>

          <div>
            <label
              for="reg-email"
              class="block text-sm font-medium text-slate-700 dark:text-slate-300">邮箱</label>
            <input
              id="reg-email"
              type="email"
              bind:value={email}
              required
              placeholder="user@example.com"
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm transition outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100" />
          </div>

          <div>
            <label
              for="reg-password"
              class="block text-sm font-medium text-slate-700 dark:text-slate-300">密码</label>
            <input
              id="reg-password"
              type="password"
              bind:value={password}
              required
              placeholder="••••••••"
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm transition outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100" />
          </div>

          <div>
            <label
              for="reg-confirm"
              class="block text-sm font-medium text-slate-700 dark:text-slate-300">确认密码</label>
            <input
              id="reg-confirm"
              type="password"
              bind:value={confirm}
              required
              placeholder="再次输入密码"
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm transition outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100" />
          </div>

          {#if siteKey}
            <div class="flex justify-center" bind:this={turnstileEl}></div>
          {/if}

          <button
            type="submit"
            disabled={loading}
            class="w-full cursor-pointer rounded-lg bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white shadow transition hover:bg-indigo-700 focus:ring-2 focus:ring-indigo-500/50 focus:outline-none disabled:cursor-not-allowed disabled:opacity-60 dark:bg-indigo-500 dark:hover:bg-indigo-600">
            {loading ? "注册中…" : "注册"}
          </button>
        </form>

        <p class="text-center text-sm text-slate-500 dark:text-slate-400">
          已有账号？<a
            href="#/"
            class="font-medium text-indigo-600 hover:text-indigo-800 dark:text-indigo-400 dark:hover:text-indigo-300"
            >去登录</a>
        </p>
      {/if}
    </div>
  </div>
</div>
