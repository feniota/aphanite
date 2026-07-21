<script lang="ts">
  import { ArrowLeft } from "@lucide/svelte";

  import "@/lib/darkmode";
  import AuthImage from "@/components/AuthImage.svelte";
  import DarkModeButton from "@/components/DarkModeButton.svelte";
  import Space from "@/components/Space.svelte";
  import {
    login,
    create_verification,
    complete_verification,
    ApiError,
    get_turnstile_site_key,
  } from "@/lib/api";
  import { AUTH } from "@/lib/auth.svelte";
  import { transition_tick } from "@/lib/utils";

  let step = $state(1);
  let email = $state("");
  let method = $state<"password" | "totp">("password");
  let password = $state("");
  let totp_code = $state("");
  let loading = $state(false);
  let error = $state("");
  let shake = $state(false);
  let public_registration: boolean = $state(false);

  function go_next(e: Event) {
    e.preventDefault();
    (document.activeElement as HTMLElement)?.blur();
    error = "";
    transition_tick(() => (step = 2));
  }

  function go_back() {
    (document.activeElement as HTMLElement)?.blur();
    error = "";
    transition_tick(() => {
      step = 1;
    });
  }

  async function handle_login(e?: SubmitEvent) {
    e?.preventDefault();
    error = "";
    loading = true;
    try {
      let otp_token: string | undefined;
      if (method === "totp") {
        const { id } = await create_verification(email, "totp");
        const res = await complete_verification(id, totp_code);
        otp_token = res.otp_token;
      }
      const result = await login(email, method === "password" ? password : undefined, otp_token);
      AUTH.set_session(result.access_token, result.user);
      window.location.href = "/";
    } catch (err) {
      transition_tick(() => {
        if (err instanceof ApiError) {
          error = err.status === 403 ? "邮箱或密码错误" : "验证失败，请重试";
        } else {
          error = "网络错误，请检查网络连接";
          console.error(err);
        }
        shake = true;
        setTimeout(() => (shake = false), 500);
      });
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    get_turnstile_site_key()
      .then(() => (public_registration = true))
      .catch(err => {
        if (err instanceof ApiError) {
          public_registration = err.status === 404;
        }
      });
  });
</script>

<div class="flex min-h-dvh flex-col items-center justify-center md:flex-row md:items-stretch">
  <div class="md:bg-background z-1 flex items-center justify-center py-12 md:flex-6 lg:flex-4">
    <div class="w-full max-w-sm overflow-hidden">
      <div
        id="page-title-container"
        class="text-center text-white drop-shadow-sm md:drop-shadow-none">
        <h1 class="dark:md:text-glaucous-200 not-dark:md:text-foreground text-3xl font-bold">
          登录
        </h1>
        <p class="md:text-muted-foreground mt-1 text-sm">
          {step !== 1 ? email || "请输入您的密码" : "欢迎回到 Aphanite"}
        </p>
      </div>

      <div
        class="bg-background/70 panel-container relative my-6 rounded-xl p-4 backdrop-blur-lg *:p-3 md:bg-transparent md:backdrop-blur-none">
        <form onsubmit={handle_login} class="space-y-2">
          <!-- Step 1: Email -->
          <div class="space-y-2 p-3" class:hidden={step !== 1}>
            <label for="login-email" class="input-label block text-sm">邮箱</label>
            <input
              id="login-email"
              type="email"
              autocomplete="username"
              bind:value={email}
              placeholder="user@example.com"
              class="input-field placeholder:text-muted-foreground input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition" />
            <button
              type="button"
              onclick={go_next}
              disabled={!email}
              class="submit-btn bg-primary disabled:text-muted-surface-foreground disabled:bg-muted mt-2 w-full rounded-lg px-3 py-2 text-sm font-semibold text-white transition-colors">
              下一步
            </button>
          </div>

          <!-- Step 2: Password -->
          <div class="p-3" class:hidden={step !== 2}>
            <!-- 隐藏邮箱，供密码管理器配对 -->
            <input
              type="email"
              value={email}
              autocomplete="username"
              class="hidden"
              readonly
              tabindex="-1" />
            <div class="flex flex-col space-y-2">
              {#if method === "password"}
                <label for="login-passwd" class="input-label block text-sm">密码</label>
                <input
                  id="login-passwd"
                  type="password"
                  autocomplete="current-password"
                  bind:value={password}
                  placeholder="·········"
                  class="input-field input-surface border-border block w-full rounded-lg border px-3 py-2 text-sm"
                  class:animate-shake={shake} />
              {:else}
                <label for="login-totp" class="input-label block text-sm">6<Space />位验证码</label>
                <input
                  id="login-totp"
                  type="text"
                  bind:value={totp_code}
                  maxlength="6"
                  placeholder="000000"
                  class="input-field input-surface border-border rounded-lg border px-3 py-2 text-sm"
                  class:animate-shake={shake} />
              {/if}
              <button
                type="submit"
                disabled={password === "" && totp_code === "" && loading}
                class="submit-btn bg-primary disabled:bg-muted mt-2 mb-2 rounded-lg px-4 py-2 text-sm font-semibold text-white">
                {loading ? "登录中…" : "登录"}
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

      {#if step === 1 && public_registration}
        <p class="md:text-foreground bottom-tip text-center text-sm text-white">
          还没有账号？
          <a
            href="#/register"
            class="text-glaucous-200 md:text-primary font-bold underline hover:underline md:font-medium md:no-underline"
            >立即注册</a>
        </p>
      {/if}
      {#if step === 2}
        <div class="bottom-tip mt-8 flex flex-col items-center">
          <div class="md:text-muted-foreground text-sm text-white">其他登录方式</div>
          <button
            type="button"
            onclick={() => {
              method = method === "password" ? "totp" : "password";
            }}
            class="md:text-primary mt-2 text-sm font-semibold text-white underline hover:underline md:no-underline"
            >{method === "password" ? "验证码登录" : "密码登录"}</button>
        </div>
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
  .bottom-tip {
    view-transition-name: bottom-tip;
  }
  .input-label {
    view-transition-name: input-label;
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
