<script lang="ts">
  import { ArrowLeft } from "@lucide/svelte";

  import "@/lib/darkmode";
  import AuthImage from "@/components/AuthImage.svelte";
  import { register, get_turnstile_site_key } from "@/lib/api";
  import { cn, transition_tick } from "@/lib/utils";

  let mode = $state("loading");
  let site_key = $state<string | null>(null);
  let register_token = $state<string | undefined>(undefined);
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
      .then(res => {
        if (res.success) {
          site_key = res.payload.site_key;
          mode = "public_turnstile";
          load_turnstile_with_retry(0);
        } else {
          mode = res.status === 404 ? "public" : "private";
        }
      })
      .catch(() => {
        mode = "error";
      });
  });

  function load_turnstile_with_retry(attempt: number) {
    if (!site_key) return;

    if (attempt > 0) {
      if (turnstile_id) {
        try {
          (window as any).turnstile?.reset(turnstile_id);
        } catch {
          /* ignore */
        }
        try {
          (window as any).turnstile?.remove(turnstile_id);
        } catch {
          /* ignore */
        }
      }
      turnstile_id = "";
      turnstile_done = false;
    }

    const existing = document.querySelector('script[src*="turnstile"]');

    const render = () => {
      clear_timer();
      const ts = (window as any).turnstile;
      const container = document.getElementById("turnstile-container");
      if (!container || !ts) {
        fallback();
        return;
      }
      try {
        const id = ts.render(container, {
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
      if ((window as any).turnstile) {
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
      s.onload = () => render();
      s.onerror = () => {
        clear_timer();
        s.remove();
        fallback();
      };
      document.head.appendChild(s);
    }
  }

  function go_step_2(e: Event) {
    e.preventDefault();
    (document.activeElement as HTMLElement)?.blur();
    error = "";
    transition_tick(() => (step = 2));
  }

  function go_back() {
    (document.activeElement as HTMLElement)?.blur();
    error = "";
    transition_tick(() => (step = 1));
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
      if (nameLen > 20) {
        error = `昵称不能长于 20 个字符，当前为 ${nameLen} 个字符`;
        shake = true;
        setTimeout(() => (shake = false), 500);
        return;
      }
    }

    loading = true;
    try {
      const ts = (window as any).turnstile;
      const res = await register({
        email,
        name: name || undefined,
        password,
        turnstile_token: turnstile_id ? ts?.getResponse(turnstile_id) : undefined,
        register_token,
      });
      if (!res.success) {
        if (res.status === 422) {
          error = "人机验证失败，请刷新页面后重试（若该错误多次出现，请联系管理员）";
        } else if (res.status === 418) {
          error = "昵称或密码格式不正确，请检查后重试";
        } else if (res.status === 409) {
          error = "该邮箱已被注册";
        } else {
          error = "未知错误";
          console.error(`Server responded with unexpected status code ${res.status}: ${res.reason}`);
        }
        shake = true;
        setTimeout(() => (shake = false), 500);
        if (turnstile_id) {
          (window as any).turnstile?.reset(turnstile_id);
          turnstile_done = false;
        }
        return;
      }
      success = true;
    } catch {
      error = "网络错误，请检查网络连接";
      shake = true;
      setTimeout(() => (shake = false), 500);
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex min-h-dvh flex-col items-center justify-center md:flex-row md:items-stretch">
  <div class="md:bg-background z-1 flex items-center justify-center py-12 md:flex-6 lg:flex-4">
    <div class="w-full max-w-sm overflow-hidden">
      <div
        id="page-title-container"
        class="text-center text-white drop-shadow-sm md:drop-shadow-none">
        <h1 class="dark:md:text-glaucous-200 not-dark:md:text-foreground text-3xl font-bold">
          注册
        </h1>
        <p class="md:text-muted-foreground mt-1 text-sm">
          {(() => {
            if (mode === "loading") {
              return "…";
            } else if (step === 1) {
              return "创建你的 Aphanite 账号";
            } else {
              return email ?? "创建你的 Aphanite 账号";
            }
          })()}
        </p>
      </div>

      {#if mode === "loading"}
        <p class="md:text-muted-foreground text-center text-sm text-white">加载中…</p>
      {:else if mode === "private" && !register_token}
        <div class="text-center">
          <p class="md:text-muted-foreground mt-6 text-sm leading-relaxed text-white">
            当前服务器未开放公开注册<br />请联系管理员获取邀请链接
          </p>
          <a href="#/" class="text-primary mt-4 inline-block text-sm font-medium hover:underline"
            >← 返回登录</a>
        </div>
      {:else if mode === "error"}
        <div class="text-center">
          <p class="md:text-muted-foreground mt-6 text-sm leading-relaxed text-white">
            无法连接服务器<br />请检查网络连接
          </p>
          <a href="#/" class="text-primary mt-4 inline-block text-sm font-medium hover:underline"
            >← 返回登录</a>
        </div>
      {:else if success}
        <div class="text-center">
          <p class="md:text-muted-foreground mt-6 text-sm leading-relaxed text-white">
            注册成功！
            <a href="#/" class="text-primary font-medium hover:underline">去登录</a>
          </p>
        </div>
      {:else}
        <div
          class="bg-background/70 panel-container relative my-6 rounded-xl p-4 backdrop-blur-lg *:p-3 md:bg-transparent md:backdrop-blur-none">
          <form onsubmit={handle_submit} class="space-y-2">
            <!-- Step 1: Email + Turnstile -->
            <div class="space-y-2 p-3" class:hidden={step !== 1}>
              <label for="reg-email" class="block text-sm">邮箱</label>
              <input
                id="reg-email"
                type="email"
                autocomplete="email"
                bind:value={email}
                required
                placeholder="user@example.com"
                class={cn(
                  "input-field placeholder:text-muted-foreground input-surface border-border mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition",
                  turnstile_id !== "" && "mb-4",
                )}
                onkeydown={(e) => e.key === "Enter" && email && (!site_key || turnstile_done) && go_step_2(e)} />
              <div
                id="turnstile-container"
                class={cn(
                  "isolate w-full overflow-hidden rounded-lg",
                  turnstile_id && "sm:min-h-16.25",
                )}>
              </div>
              <button
                type="button"
                onclick={go_step_2}
                disabled={!email || (!!site_key && !turnstile_done)}
                class="submit-btn bg-primary disabled:bg-muted disabled:text-muted-surface-foreground mt-2 w-full rounded-lg px-3 py-2 text-sm font-semibold text-white transition-colors">
                下一步
              </button>
            </div>

            <!-- Step 2: Username + Password -->
            <div class="p-3" class:hidden={step !== 2}>
              <!-- 隐形邮箱，让密码管理器认这个而不是昵称 -->
              <input
                type="email"
                value={email}
                autocomplete="username"
                readonly
                tabindex="-1"
                class="hidden" />
              <div class="flex flex-col space-y-2">
                <label for="reg-usr-xxxxxxxx" class="block text-sm">昵称</label>
                <input
                  id="reg-usr-xxxxxxxx"
                  type="text"
                  autocomplete="off"
                  bind:value={name}
                  placeholder="一般路过 Minecraft 玩家"
                  class="placeholder:text-muted-foreground input-surface border-border mb-3 block w-full rounded-lg border px-3 py-2 text-sm transition" />
                <label for="reg-password" class="block text-sm">密码</label>
                <input
                  autocomplete="new-password"
                  id="reg-password"
                  type="password"
                  bind:value={password}
                  required
                  placeholder="·········"
                  class="placeholder:text-muted-foreground input-surface border-border mb-3 block w-full rounded-lg border px-3 py-2 text-sm transition"
                  class:animate-shake={shake} />
                <label for="reg-confirm" class="block text-sm">确认密码</label>
                <input
                  autocomplete="new-password"
                  id="reg-confirm"
                  type="password"
                  bind:value={confirm}
                  required
                  placeholder="·········"
                  class="placeholder:text-muted-foreground input-surface border-border block w-full rounded-lg border px-3 py-2 text-sm transition"
                  class:animate-shake={shake} />
                <button
                  type="submit"
                  disabled={loading}
                  class="submit-btn bg-primary disabled:bg-muted mt-2 w-full rounded-lg px-3 py-2 text-sm font-semibold text-white transition-colors">
                  {loading ? "注册中…" : "注册"}
                </button>
                <button
                  type="button"
                  onclick={go_back}
                  class="text-muted-foreground hover:text-primary mt-2 flex items-center text-sm transition-colors">
                  <ArrowLeft class="size-4" />
                  <div>上一步</div>
                </button>
              </div>
            </div>
          </form>
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
  #page-title-container {
    view-transition-name: page-title;
  }
  .input-field {
    view-transition-name: input-field;
  }
  .submit-btn {
    view-transition-name: submit-btn;
  }
  .panel-container {
    view-transition-name: panel-container;
  }
</style>
