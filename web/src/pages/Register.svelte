<script lang="ts">
  import { tick } from "svelte";
  import { ArrowLeft } from "@lucide/svelte";

  import "@/lib/darkmode";
  import AuthImage from "@/components/AuthImage.svelte";
  import { register, get_turnstile_site_key, ApiError } from "@/lib/api";

  let mode = $state("loading");
  let site_key = $state<string | null>(null);
  let register_token = $state<string | undefined>(undefined);
  let turnstile_el = $state<HTMLDivElement | null>(null);
  let turnstile_id = $state("");
  let turnstile_done = $state(false);
  let step = $state(1);

  let email = $state("");
  let name = $state("");
  let password = $state("");
  let confirm = $state("");
  let loading = $state(false);
  let error = $state("");
  let shake = $state(false);
  let success = $state(false);

  const TURNSTILE_TIMEOUT_MS = 5_000;
  const TURNSTILE_MAX_RETRIES = 2;

  $effect(() => {
    const p = new URLSearchParams(window.location.search);
    register_token = p.get("token") || undefined;

    get_turnstile_site_key()
      .then(async ({ site_key: sk }) => {
        site_key = sk;
        mode = "public_turnstile";
        await tick();
        await tick();
        load_turnstile_with_retry(0);
      })
      .catch(err => {
        if (err instanceof ApiError) {
          mode = err.status === 404 ? "public" : "private";
        } else {
          mode = "error";
        }
      });
  });

  function load_turnstile_with_retry(attempt: number) {
    if (!site_key || !turnstile_el) return;
    if (attempt > 0) {
      const ts = (window as any).turnstile;
      if (turnstile_id && ts) {
        try {
          ts.reset(turnstile_id);
        } catch {
          /* ignore */
        }
        try {
          ts.remove(turnstile_id);
        } catch {
          /* ignore */
        }
      }
      turnstile_id = "";
      turnstile_done = false;
    }

    const ts = (window as any).turnstile;
    const existing = document.querySelector('script[src*="turnstile"]');

    const render = () => {
      clear_timer();
      try {
        const id = ts?.render(turnstile_el!, {
          sitekey: site_key,
          size: "flexible",
          theme: window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light",
          callback: () => (turnstile_done = true),
        });
        if (id) {
          turnstile_id = id;
          error = "";
        }
      } catch {
        error = "安全验证加载失败，正在重试…";
      }
    };

    const fallback = () => {
      if (attempt < TURNSTILE_MAX_RETRIES) {
        error = "安全验证加载失败，正在重试…";
        load_turnstile_with_retry(attempt + 1);
      } else {
        error = "安全验证加载失败，请刷新页面重试";
      }
    };

    let timed_out = false;
    const timeout_id = setTimeout(() => {
      timed_out = true;
      fallback();
    }, TURNSTILE_TIMEOUT_MS);

    const clear_timer = () => {
      if (!timed_out) clearTimeout(timeout_id);
    };

    if (existing) {
      if (ts) {
        render();
      } else {
        existing.addEventListener(
          "load",
          () => {
            render();
          },
          { once: true },
        );
      }
    } else {
      const s = document.createElement("script");
      s.src = "https://challenges.cloudflare.com/turnstile/v0/api.js";
      s.async = true;
      s.defer = true;
      s.onload = () => {
        render();
      };
      s.onerror = () => {
        clear_timer();
        s.remove();
        fallback();
      };
      document.head.appendChild(s);
    }
  }

  function go_step_2(e: SubmitEvent) {
    e.preventDefault();
    (document.activeElement as HTMLElement)?.blur();
    error = "";
    step++;
  }

  function go_back() {
    (document.activeElement as HTMLElement)?.blur();
    step--;
    error = "";
  }

  async function handle_submit(e: SubmitEvent) {
    e.preventDefault();
    error = "";

    if (password !== confirm) {
      error = "两次输入的密码不一致";
      shake = true;
      setTimeout(() => (shake = false), 500);
      return;
    }

    if (password.length < 8) {
      error = `密码长度不能少于 8 个字符，当前为 ${password.length} 个字符`;
      shake = true;
      setTimeout(() => (shake = false), 500);
      return;
    }
    if (password.length > 128) {
      error = `密码长度不能超过 128 个字符，当前为 ${password.length} 个字符`;
      shake = true;
      setTimeout(() => (shake = false), 500);
      return;
    }

    if (name) {
      const nameLen = [...name].length;
      if (nameLen < 3 || nameLen > 16) {
        error = `昵称长度需为 3–16 个字符，当前为 ${nameLen} 个字符`;
        shake = true;
        setTimeout(() => (shake = false), 500);
        return;
      }
      if (!/^[a-zA-Z0-9_-]+$/.test(name)) {
        error = "昵称只能包含字母、数字、下划线和连字符（-）";
        shake = true;
        setTimeout(() => (shake = false), 500);
        return;
      }
    }

    loading = true;
    try {
      const ts = (window as any).turnstile;
      await register({
        email,
        name: name || undefined,
        password,
        turnstile_token: turnstile_id ? ts?.getResponse(turnstile_id) : undefined,
        register_token,
      });
      success = true;
    } catch (err) {
      if (err instanceof ApiError) {
        error =
          err.status === 422
            ? "昵称或密码格式不正确，请检查后重试"
            : err.status === 409
              ? "该邮箱已被注册"
              : "注册失败，请重试";
      } else {
        error = "网络错误，请检查网络连接";
      }
      shake = true;
      setTimeout(() => (shake = false), 500);
      if (turnstile_id) {
        (window as any).turnstile?.reset(turnstile_id);
        turnstile_done = false;
      }
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex min-h-dvh flex-col items-center justify-center md:flex-row md:items-stretch">
  <div class="md:bg-background z-1 flex items-center justify-center py-12 md:flex-6 lg:flex-4">
    <div class="w-full max-w-sm overflow-hidden">
      <div class="text-center text-white drop-shadow-sm md:drop-shadow-none">
        <h1 class="dark:md:text-glaucous-200 not-dark:md:text-foreground text-3xl font-bold">
          注册
        </h1>
        <p class="md:text-muted-foreground mt-1 text-sm">
          {mode === "loading" ? "…" : "创建你的 Aphanite 账号"}
        </p>
      </div>

      {#if mode === "loading"}
        <p class="text-center text-sm text-white md:text-muted-foreground">加载中…</p>
      {:else if mode === "private" && !register_token}
        <div class="text-center">
          <p class="mt-6 text-sm leading-relaxed text-white md:text-muted-foreground">
            当前服务器未开放公开注册<br />请联系管理员获取邀请链接
          </p>
          <a
            href="#/"
            class="text-primary mt-4 inline-block text-sm font-medium hover:underline"
            >← 返回登录</a>
        </div>
      {:else if mode === "error"}
        <div class="text-center">
          <p class="mt-6 text-sm leading-relaxed text-white md:text-muted-foreground">
            无法连接服务器<br />请检查网络连接
          </p>
          <a
            href="#/"
            class="text-primary mt-4 inline-block text-sm font-medium hover:underline"
            >← 返回登录</a>
        </div>
      {:else if success}
        <div class="text-center">
          <p class="mt-6 text-sm leading-relaxed text-white md:text-muted-foreground">
            注册成功！
            <a
              href="#/"
              class="text-primary font-medium hover:underline"
              >去登录</a>
          </p>
        </div>
      {:else}
        <div
          class="bg-background/70 relative my-6 rounded-xl p-4 backdrop-blur-lg *:p-3 md:bg-transparent">
          <!-- Step 1: Email + Turnstile -->
          <div
            class="transition-all duration-300"
            class:translate-x-[-120%]={step > 1}
            class:opacity-0={step > 1}
            class:absolute={step > 1}
            class:inset-0={step > 1}
            inert={step > 1}>
            <form onsubmit={go_step_2} class="space-y-2">
              <div>
                <label for="reg-email" class="block text-sm">邮箱</label>
                <input
                  id="reg-email"
                  type="email"
                  bind:value={email}
                  required
                  placeholder="user@example.com"
                  class="placeholder:text-muted-foreground bg-surface mt-1 block w-full rounded-lg border border-border px-3 py-2 text-sm transition" />
              </div>
              {#if site_key}
                <div class="w-full overflow-hidden rounded-lg" bind:this={turnstile_el}></div>
              {/if}
              <button
                type="submit"
                disabled={!email || (!!site_key && !turnstile_done)}
                class="bg-primary disabled:bg-muted disabled:text-muted-surface-foreground mt-2 w-full rounded-lg px-3 py-2 text-sm font-semibold text-white transition-colors">
                下一步
              </button>
            </form>
          </div>

          <!-- Step 2: Username + Password -->
          <div
            class="transition-all duration-300"
            class:translate-x-full={step < 2}
            class:opacity-0={step < 2}
            class:absolute={step < 2}
            class:inset-0={step < 2}
            inert={step < 2}>
            <p class="text-sm text-white md:text-muted-foreground">{email}</p>
            <form onsubmit={handle_submit} class="space-y-2">
              <div>
                <label for="reg-username" class="block text-sm">用户名</label>
                <input
                  id="reg-username"
                  type="text"
                  bind:value={name}
                  placeholder="User 玩家名"
                  class="placeholder:text-muted-foreground bg-surface mt-1 block w-full rounded-lg border border-border px-3 py-2 text-sm transition" />
              </div>
              <div>
                <label for="reg-password" class="block text-sm">密码</label>
                <input
                  id="reg-password"
                  type="password"
                  bind:value={password}
                  required
                  placeholder="·········"
                  class="placeholder:text-muted-foreground bg-surface mt-1 block w-full rounded-lg border border-border px-3 py-2 text-sm transition"
                  class:animate-shake={shake} />
              </div>
              <div>
                <label for="reg-confirm" class="block text-sm">确认密码</label>
                <input
                  id="reg-confirm"
                  type="password"
                  bind:value={confirm}
                  required
                  placeholder="·········"
                  class="placeholder:text-muted-foreground bg-surface mt-1 block w-full rounded-lg border border-border px-3 py-2 text-sm transition"
                  class:animate-shake={shake} />
              </div>
              <button
                type="submit"
                disabled={loading}
                class="bg-primary disabled:bg-muted mt-2 w-full rounded-lg px-3 py-2 text-sm font-semibold text-white transition-colors">
                {loading ? "注册中…" : "注册"}
              </button>
              <button
                type="button"
                onclick={go_back}
                class="text-muted-foreground hover:text-primary mt-2 flex items-center text-sm transition-colors">
                <ArrowLeft class="size-4" />
                <div>上一步</div>
              </button>
            </form>
          </div>
        </div>

        {#if error}
          <p class="h-5 text-center text-sm text-red-400">{error}</p>
        {/if}

        {#if step === 1}
          <p class="md:text-foreground text-center text-sm text-white">
            已有账号？
            <a
              href="#/"
              class="text-glaucous-200 md:text-primary font-bold underline hover:underline md:font-medium md:no-underline"
              >去登录</a>
          </p>
        {/if}
      {/if}
    </div>
  </div>
  <div
    class="bg-glaucous-200 dark:bg-glaucous-900 absolute h-dvh w-auto flex-12 items-center justify-center self-stretch md:relative md:block">
    <AuthImage />
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
