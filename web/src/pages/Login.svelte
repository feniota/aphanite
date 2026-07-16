<script lang="ts">
  import { login, createVerification, completeVerification, ApiError } from "../lib/api";
  import { auth } from "../lib/auth.svelte";
  import AuthImage from "../lib/AuthImage.svelte";

  let step = $state(1);
  let direction = $state<"forward" | "back">("forward");
  let email = $state("");
  let method = $state<"password" | "totp">("password");
  let password = $state("");
  let totpCode = $state("");
  let loading = $state(false);
  let error = $state("");

  function goNext() {
    direction = "forward";
    step++;
    error = "";
  }

  function goBack() {
    direction = "back";
    step--;
    error = "";
  }

  async function handleLogin() {
    error = "";
    loading = true;
    try {
      let otp_token: string | undefined;
      if (method === "totp") {
        const { id } = await createVerification(email, "totp");
        const res = await completeVerification(id, totpCode);
        otp_token = res.otp_token;
      }
      const result = await login(email, method === "password" ? password : undefined, otp_token);
      auth.setSession(result.access_token, result.user);
      window.location.href = "/";
    } catch (err) {
      error = err instanceof ApiError ? err.message : "网络错误，请检查后端是否已启动";
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
        <h1 class="text-3xl font-bold text-slate-900 dark:text-slate-100">登录</h1>
        <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">欢迎回到 aphanite</p>
      </div>

      <div class="relative">
        <!-- Step 1: Email -->
        <div
          class="transition-all duration-300"
          class:translate-x-[-120%]={step > 1}
          class:opacity-0={step > 1}
          class:absolute={step > 1}
          class:inset-0={step > 1}>
          <div class="space-y-4">
            <div>
              <label
                for="login-email"
                class="block text-sm font-medium text-slate-700 dark:text-slate-300">邮箱</label>
              <input
                id="login-email"
                type="email"
                bind:value={email}
                placeholder="user@example.com"
                class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm transition outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100" />
            </div>
            <button
              onclick={goNext}
              disabled={!email}
              class="w-full cursor-pointer rounded-lg bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white shadow transition hover:bg-indigo-700 disabled:cursor-not-allowed disabled:opacity-60 dark:bg-indigo-500 dark:hover:bg-indigo-600">
              下一步
            </button>
          </div>
        </div>

        <!-- Step 2: Choose method -->
        <div
          class="transition-all duration-300"
          class:translate-x-full={step < 2}
          class:translate-x-[-120%]={step > 2}
          class:opacity-0={step !== 2}
          class:absolute={step !== 2}
          class:inset-0={step !== 2}>
          <p class="text-sm text-slate-500 dark:text-slate-400">{email}</p>
          <p class="mt-3 text-sm text-slate-500 dark:text-slate-400">选择验证方式</p>
          <div class="mt-3 space-y-3">
            <button
              onclick={() => {
                method = "password";
                goNext();
              }}
              class="w-full cursor-pointer rounded-xl border-2 border-slate-200 p-4 text-left transition hover:border-slate-300 dark:border-slate-600 dark:hover:border-slate-500">
              <span class="text-sm font-medium text-slate-900 dark:text-slate-100">密码</span>
              <span class="mt-1 block text-xs text-slate-500 dark:text-slate-400"
                >使用密码登录</span>
            </button>
            <button
              onclick={() => {
                method = "totp";
                goNext();
              }}
              class="w-full cursor-pointer rounded-xl border-2 border-slate-200 p-4 text-left transition hover:border-slate-300 dark:border-slate-600 dark:hover:border-slate-500">
              <span class="text-sm font-medium text-slate-900 dark:text-slate-100">TOTP</span>
              <span class="mt-1 block text-xs text-slate-500 dark:text-slate-400"
                >使用认证器应用生成的验证码</span>
            </button>
          </div>
          <div class="mt-6 flex justify-between">
            <button
              onclick={goBack}
              class="cursor-pointer rounded-lg border border-slate-300 px-4 py-2 text-sm text-slate-600 dark:border-slate-600 dark:text-slate-400">
              ← 上一步
            </button>
          </div>
        </div>

        <!-- Step 3: Verify -->
        <div
          class="transition-all duration-300"
          class:translate-x-full={step < 3}
          class:opacity-0={step < 3}
          class:absolute={step < 3}
          class:inset-0={step < 3}>
          <p class="text-sm text-slate-500 dark:text-slate-400">{email}</p>
          <p class="mt-3 text-sm text-slate-500 dark:text-slate-400">
            验证方式：{method === "password" ? "密码" : "TOTP"}
          </p>
          {#if method === "password"}
            <input
              type="password"
              bind:value={password}
              placeholder="输入密码"
              class="mt-3 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm transition outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100" />
          {:else}
            <div class="mt-3 flex items-center gap-2">
              <span class="text-sm text-slate-500 dark:text-slate-400">输入 6 位验证码：</span>
              <input
                type="text"
                bind:value={totpCode}
                maxlength="6"
                placeholder="000000"
                class="w-28 rounded-lg border border-slate-300 px-3 py-2 text-center text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100" />
            </div>
          {/if}
          <div class="mt-6 flex justify-between">
            <button
              onclick={goBack}
              class="cursor-pointer rounded-lg border border-slate-300 px-4 py-2 text-sm text-slate-600 dark:border-slate-600 dark:text-slate-400">
              ← 上一步
            </button>
            <button
              onclick={handleLogin}
              disabled={loading}
              class="cursor-pointer rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-700 disabled:opacity-60 dark:bg-indigo-500 dark:hover:bg-indigo-600">
              {loading ? "登录中…" : "登录"}
            </button>
          </div>
        </div>
      </div>

      {#if error}
        <p class="text-center text-sm text-red-600 dark:text-red-400">{error}</p>
      {/if}

      {#if step === 1}
        <p class="text-center text-sm text-slate-500 dark:text-slate-400">
          还没有账号？
          <a
            href="#/register"
            class="font-medium text-indigo-600 hover:text-indigo-800 dark:text-indigo-400 dark:hover:text-indigo-300"
            >立即注册</a>
        </p>
      {/if}
    </div>
  </div>
</div>
