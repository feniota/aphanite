<script lang="ts">
  import { ArrowLeft } from "@lucide/svelte";

  import "@/lib/darkmode";
  import { onMount } from "svelte";

  import AuthImage from "@/components/AuthImage.svelte";
  import DarkModeButton from "@/components/DarkModeButton.svelte";
  import LangSwitcher from "@/components/LangSwitcher.svelte";
  import Toast from "@/components/Toast.svelte";
  import { toast } from "@/components/toast.svelte";
  import {
    login,
    create_verification,
    complete_verification,
    get_turnstile_site_key,
  } from "@/lib/api";
  import { AUTH } from "@/lib/auth.svelte";
  import { t } from "@/lib/i18n.svelte";
  import Trans from "@/lib/Trans.svelte";
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
        const verification = await create_verification(email, "totp");
        if (!verification.success) {
          transition_tick(() => {
            error = t("login.error_totp_send_failed");
            shake = true;
            setTimeout(() => (shake = false), 500);
          });
          return;
        }
        const complete_res = await complete_verification(verification.payload.id, totp_code);
        if (!complete_res.success) {
          transition_tick(() => {
            error = t("login.error_totp_failed");
            shake = true;
            setTimeout(() => (shake = false), 500);
          });
          return;
        }
        otp_token = complete_res.payload.otp_token;
      }
      const result = await login(email, method === "password" ? password : undefined, otp_token);
      if (!result.success) {
        transition_tick(() => {
          error =
            result.status === 403
              ? t("login.error_wrong_credentials")
              : t("login.error_verification_failed");
          shake = true;
          setTimeout(() => (shake = false), 500);
        });
        return;
      }
      AUTH.set_session(result.payload.access_token, result.payload.user);
      window.location.href = "/";
    } catch {
      transition_tick(() => {
        error = t("login.error_network");
        shake = true;
        setTimeout(() => (shake = false), 500);
      });
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    get_turnstile_site_key()
      .then(res => {
        public_registration = res.success || res.status === 404;
      })
      .catch(() => {});
  });

  onMount(() => {
    if (new URLSearchParams(window.location.search).get("redirected_from_dashboard") === "true") {
      toast(t("toast.session_expired"));
    }
  });
</script>

<div class="flex min-h-dvh flex-col items-center justify-center md:flex-row md:items-stretch">
  <div class="md:bg-background z-1 flex items-center justify-center py-12 md:flex-6 lg:flex-4">
    <div class="w-full max-w-sm overflow-hidden">
      <div
        id="page-title-container"
        class="text-center text-white drop-shadow-sm md:drop-shadow-none">
        <h1 class="dark:md:text-glaucous-200 not-dark:md:text-foreground text-3xl font-bold">
          <Trans k="login.title" />
        </h1>
        <p class="md:text-muted-foreground mt-1 text-sm">
          {#if step !== 1}
            {email || t("login.enter_password")}
          {:else}
            <Trans k="login.welcome_back" />
          {/if}
        </p>
      </div>

      <div
        class="bg-background/70 panel-container relative my-6 rounded-xl p-4 backdrop-blur-lg *:p-3 md:bg-transparent md:backdrop-blur-none">
        <form onsubmit={handle_login} class="space-y-2">
          <!-- Step 1: Email -->
          <div class="space-y-2 p-3" class:hidden={step !== 1}>
            <label for="login-email" class="input-label block text-sm">{t("common.email")}</label>
            <input
              id="login-email"
              type="email"
              autocomplete="username"
              bind:value={email}
              placeholder="user@example.com"
              class="input-field placeholder:text-muted-foreground input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition"
              required
              onkeydown={e => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  if (email) go_next(e);
                }
              }} />
            <button
              type="button"
              onclick={go_next}
              disabled={!email}
              class="submit-btn bg-primary disabled:text-muted-surface-foreground disabled:bg-muted mt-2 w-full rounded-lg px-3 py-2 text-sm font-semibold text-white transition-colors">
              {t("login.next")}
            </button>
          </div>

          <!-- Step 2: Password -->
          <div class="p-3" class:hidden={step !== 2}>
            <!-- Hidden email for password manager pairing -->
            <input
              type="email"
              value={email}
              autocomplete="username"
              class="hidden"
              readonly
              tabindex="-1" />
            <div class="flex flex-col space-y-2">
              {#if method === "password"}
                <label for="login-passwd" class="input-label block text-sm"
                  >{t("common.password")}</label>
                <input
                  id="login-passwd"
                  type="password"
                  autocomplete="current-password"
                  bind:value={password}
                  placeholder="·········"
                  class="input-field input-surface border-border block w-full rounded-lg border px-3 py-2 text-sm"
                  class:animate-shake={shake} />
              {:else}
                <label for="login-totp" class="input-label block text-sm"
                  ><Trans k="login.totp_code_label" /></label>
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
                {loading ? t("login.logging_in") : t("login.login")}
              </button>
              <button
                type="button"
                onclick={go_back}
                class="text-muted-foreground hover:text-primary mt-2 flex items-center text-sm transition-colors">
                <ArrowLeft class="size-4" />
                <div>{t("common.back")}</div>
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
          {t("login.no_account")}
          <a
            href="#/register"
            class="text-glaucous-200 md:text-primary font-bold underline hover:underline md:font-medium md:no-underline"
            >{t("login.register")}</a>
        </p>
      {/if}
      {#if step === 2}
        <div class="bottom-tip mt-8 flex flex-col items-center">
          <div class="md:text-muted-foreground text-sm text-white">{t("login.other_methods")}</div>
          <button
            type="button"
            onclick={() => {
              method = method === "password" ? "totp" : "password";
            }}
            class="md:text-primary mt-2 text-sm font-semibold text-white underline hover:underline md:no-underline"
            >{method === "password" ? t("login.totp_login") : t("login.password_login")}</button>
        </div>
      {/if}

      <div class="mt-6 flex justify-center">
        <LangSwitcher />
      </div>
    </div>
  </div>
  <div
    class="bg-glaucous-200 dark:bg-glaucous-900 absolute h-dvh w-auto flex-9 items-center justify-center self-stretch md:relative md:block">
    <AuthImage />
  </div>
</div>
<Toast></Toast>

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
