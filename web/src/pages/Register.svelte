<script lang="ts">
  import { tick } from "svelte";

  import { register, getTurnstileSiteKey, ApiError } from "../lib/api";
  import AuthImage from "../lib/AuthImage.svelte";

  let mode = $state("loading");
  let siteKey = $state<string | null>(null);
  let registerToken = $state<string | undefined>(undefined);
  let turnstileEl = $state<HTMLDivElement | null>(null);
  let turnstileId = $state("");
  let step = $state(1);

  let email = $state("");
  let name = $state("");
  let password = $state("");
  let confirm = $state("");
  let loading = $state(false);
  let error = $state("");
  let shake = $state(false);
  let success = $state(false);

  $effect(() => {
    const p = new URLSearchParams(window.location.search);
    registerToken = p.get("token") || undefined;

    getTurnstileSiteKey()
      .then(async ({ site_key }) => {
        siteKey = site_key;
        mode = "public_turnstile";
        await tick();
        loadTurnstile();
      })
      .catch((err) => {
        if (err instanceof ApiError) {
          mode = err.status === 404 ? "public" : "private";
        } else {
          mode = "error";
        }
      });
  });

  function loadTurnstile() {
    if (!siteKey || !turnstileEl) return;
    const ts = (window as any).turnstile;
    const existing = document.querySelector('script[src*="turnstile"]');

    const render = () => {
      const id = ts?.render(turnstileEl!, {
        sitekey: siteKey,
        size: "flexible",
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
  }

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    error = "";

    if (password !== confirm) {
      error = "两次输入的密码不一致";
      shake = true;
      setTimeout(() => (shake = false), 500);
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
      error = err instanceof ApiError ? "注册失败，请重试" : "网络错误，请检查网络连接";
      shake = true;
      setTimeout(() => (shake = false), 500);
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
    <div class="w-full max-w-sm space-y-6 overflow-hidden">
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
            无法连接服务器<br />请检查网络连接
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
        <div class="relative">
          <!-- Step 1: Email + Turnstile -->
          <div
            class="transition-all duration-300"
            class:translate-x-[-120%]={step > 1}
            class:opacity-0={step > 1}
            class:absolute={step > 1}
            class:inset-0={step > 1}>
            <div class="space-y-4">
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
              {#if siteKey}
                <div class="w-full overflow-hidden rounded-lg" bind:this={turnstileEl}></div>
              {/if}
              <button
                onclick={() => {
                  error = "";
                  step++;
                }}
                disabled={!email}
                class="w-full cursor-pointer rounded-lg bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white shadow transition hover:bg-indigo-700 disabled:cursor-not-allowed disabled:opacity-60 dark:bg-indigo-500 dark:hover:bg-indigo-600">
                下一步
              </button>
            </div>
          </div>

          <!-- Step 2: Username + Password -->
          <div
            class="transition-all duration-300"
            class:translate-x-full={step < 2}
            class:opacity-0={step < 2}
            class:absolute={step < 2}
            class:inset-0={step < 2}>
            <p class="text-sm text-slate-500 dark:text-slate-400">{email}</p>
            <form class="mt-3 space-y-4" onsubmit={handleSubmit}>
              <div>
                <label
                  for="reg-username"
                  class="block text-sm font-medium text-slate-700 dark:text-slate-300"
                  >用户名</label>
                <input
                  id="reg-username"
                  type="text"
                  bind:value={name}
                  placeholder="User 玩家名"
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
                  class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm transition outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100"
                  class:animate-shake={shake} />
              </div>
              <div>
                <label
                  for="reg-confirm"
                  class="block text-sm font-medium text-slate-700 dark:text-slate-300"
                  >确认密码</label>
                <input
                  id="reg-confirm"
                  type="password"
                  bind:value={confirm}
                  required
                  placeholder="再次输入密码"
                  class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm transition outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100"
                  class:animate-shake={shake} />
              </div>
              <div class="flex justify-between">
                <button
                  type="button"
                  onclick={() => {
                    step--;
                    error = "";
                  }}
                  class="cursor-pointer rounded-lg border border-slate-300 px-4 py-2 text-sm text-slate-600 dark:border-slate-600 dark:text-slate-400"
                  >← 上一步</button>
                <button
                  type="submit"
                  disabled={loading}
                  class="cursor-pointer rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-700 disabled:opacity-60 dark:bg-indigo-500 dark:hover:bg-indigo-600">
                  {loading ? "注册中…" : "注册"}
                </button>
              </div>
            </form>
          </div>
        </div>

        {#if error}
          <p class="h-5 text-center text-sm text-slate-500 dark:text-slate-400">{error}</p>
        {/if}

        {#if step === 1}
          <p class="text-center text-sm text-slate-500 dark:text-slate-400">
            已有账号？<a
              href="#/"
              class="font-medium text-indigo-600 hover:text-indigo-800 dark:text-indigo-400 dark:hover:text-indigo-300"
              >去登录</a>
          </p>
        {/if}
      {/if}
    </div>
  </div>
</div>

<style>
  .animate-shake {
    animation: shake 0.4s ease-in-out;
  }
  @keyframes shake {
    0%,
    100% {
      transform: translateX(0);
    }
    25% {
      transform: translateX(-6px);
    }
    50% {
      transform: translateX(6px);
    }
    75% {
      transform: translateX(-4px);
    }
  }
</style>
