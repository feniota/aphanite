<script lang="ts">
  import { ChevronRight, LoaderCircle, Copy, Check } from "@lucide/svelte";
  import { toDataURL } from "qrcode";
  import { tick } from "svelte";

  import Dialog from "@/components/Dialog.svelte";
  import { toast } from "@/components/toast.svelte";
  import {
    change_password,
    update_me,
    issue_totp,
    delete_totp,
    create_verification,
    complete_verification,
  } from "@/lib/api";
  import { AUTH } from "@/lib/auth.svelte";
  import * as Tooltip from "@/lib/components/ui/tooltip";
  import { get_dark_mode } from "@/lib/darkmode";
  import { t } from "@/lib/i18n.svelte";
  import Trans from "@/lib/Trans.svelte";
  import { transition_tick } from "@/lib/utils";
  import Management from "@/pages/Management.svelte";

  // ── Reset Password ──
  let password_dialog: null | Dialog = $state(null);
  let old_password = $state("");
  let new_password = $state("");
  let confirm_password = $state("");
  let password_loading = $state(false);
  let password_error = $state("");

  function reset_password_state() {
    old_password = "";
    new_password = "";
    confirm_password = "";
    password_loading = false;
    password_error = "";
  }

  async function handle_change_password(e: SubmitEvent) {
    e.preventDefault();
    password_error = "";

    if (new_password.length < 8) {
      password_error = t("user.error_password_short");
      return;
    }
    if (new_password !== confirm_password) {
      password_error = t("user.error_password_mismatch");
      return;
    }

    if (!AUTH.token) return;
    password_loading = true;
    const resp = await change_password(AUTH.token, {
      old_password: old_password || undefined,
      new_password,
    });
    password_loading = false;
    if (!resp.success) {
      password_error = resp.reason;
      return;
    }
    toast(t("toast.password_updated"));
    password_dialog?.close();
    reset_password_state();
  }

  // ── Update Account Info ──
  let user_dialog: null | Dialog = $state(null);
  let user_name = $state(AUTH.user?.name ?? "");
  let user_email = $state(AUTH.user?.email ?? "");
  let user_loading = $state(false);
  let user_error = $state("");

  function reset_user_state() {
    user_name = AUTH.user?.name ?? "";
    user_email = AUTH.user?.email ?? "";
    user_loading = false;
    user_error = "";
  }

  async function handle_update_user(e: SubmitEvent) {
    e.preventDefault();
    user_error = "";
    if (!AUTH.token) return;

    const name = user_name.trim();
    const email = user_email.trim();

    if (!name && !email) {
      user_error = t("user.error_missing_field");
      return;
    }

    user_loading = true;
    const resp = await update_me(AUTH.token, {
      name: name || undefined,
      email: email || undefined,
    });
    user_loading = false;
    if (!resp.success) {
      user_error = resp.reason;
      return;
    }
    toast(t("toast.user_updated"));
    AUTH.user = resp.payload;
    localStorage.setItem("aphanite_user", JSON.stringify(resp.payload));
    user_dialog?.close();
  }

  // ── TOTP ──
  let totp_dialog: null | Dialog = $state(null);
  let totp_step: "loading" | "verify" = $state("loading");
  let totp_secret = $state("");
  let totp_otpauth_url = $state("");
  let totp_qr_data_url = $state("");
  let totp_code = $state("");
  let totp_loading = $state(false);
  let totp_error = $state("");
  let totp_copied = $state(false);
  let totp_code_input: HTMLInputElement | null = $state(null);

  // Autofocus the TOTP code input when the dialog transitions to verify step
  $effect(() => {
    if (totp_step === "verify" && totp_code_input) {
      totp_code_input.focus();
    }
  });

  function reset_totp_state() {
    totp_step = "loading";
    totp_secret = "";
    totp_otpauth_url = "";
    totp_qr_data_url = "";
    totp_code = "";
    totp_loading = false;
    totp_error = "";
    totp_copied = false;
  }

  async function open_totp_dialog() {
    reset_totp_state();
    totp_dialog?.open();

    if (!AUTH.token) return;
    transition_tick(async () => {
      const resp = await issue_totp(AUTH.token!);
      if (!resp.success) {
        totp_error = resp.reason;
        totp_step = "verify";
        return;
      }
      totp_secret = resp.payload.secret;
      totp_otpauth_url = resp.payload.otpauth_url;
      const dark = get_dark_mode();
      try {
        totp_qr_data_url = await toDataURL(totp_otpauth_url, {
          width: 256,
          margin: 2,
          color: {
            dark: dark
              ? "#0d1117" // dark:color-background (glaucous-950)
              : "#4a6282", // glaucous-600
            light: dark
              ? "#9eb0c7" // dark:color-foreground (glaucous-300)
              : "#eff2f6", // not-dark:color-foreground (glaucous-50)
          },
        });
      } catch (e) {
        console.error("生成二维码失败：", e);
      }
      totp_step = "verify";
    });
  }

  async function handle_activate_totp() {
    if (!AUTH.token || !AUTH.user?.email) return;
    totp_error = "";
    if (!totp_code) {
      totp_error = t("user.error_enter_code");
      return;
    }
    totp_loading = true;

    // Create a TOTP verification session to validate the user has set up correctly
    const session = await create_verification(AUTH.user.email, "totp");
    if (!session.success) {
      totp_loading = false;
      totp_error = session.reason;
      return;
    }

    const resp = await complete_verification(session.payload.id, totp_code);
    totp_loading = false;
    if (!resp.success) {
      totp_error = resp.reason;
      return;
    }

    toast(t("toast.totp_activated"));
    totp_dialog?.close();
    reset_totp_state();
  }

  async function handle_disable_totp(hide_toast?: boolean) {
    if (!AUTH.token) return;
    totp_loading = true;
    const resp = await delete_totp(AUTH.token);
    totp_loading = false;
    if (!resp.success) {
      totp_error = resp.reason;
      return;
    }
    if (!hide_toast) toast(t("toast.totp_disabled"));
    totp_dialog?.close();
    reset_totp_state();
  }

  async function copy_totp_secret() {
    try {
      await navigator.clipboard.writeText(totp_secret);
      toast(t("common.copied"));
      totp_copied = true;
      setTimeout(() => (totp_copied = false), 1000);
    } catch (e) {
      toast(t("common.copy_failed"));
      console.error(e);
    }
  }

  // ── Logout ──
  let logout_dialog: null | Dialog = $state(null);

  function handle_logout() {
    AUTH.logout();
    window.location.href = "/login";
  }
</script>

<div class="flex w-full flex-col gap-4">
  <div class="title">{t("user.title")}</div>
  <div class="grid grid-cols-[auto_1fr] rounded-lg border *:px-4 *:py-3 *:odd:border-r">
    <span>{t("user.nickname")}</span><span>{AUTH.user?.name}</span>
    <span>{t("user.email")}</span><span>{AUTH.user?.email}</span>
    <span>{t("common.uuid")}</span><span class="font-mono">{AUTH.user?.id}</span>
  </div>
  <div class="text-primary-foreground border-b py-4 text-lg">{t("user.manage_account")}</div>
  <div class="-mt-4 flex flex-col divide-y border-b *:p-4 *:focus:ring-0">
    <button
      onclick={() => password_dialog?.open()}
      class="hover:bg-surface/50 flex flex-row items-center justify-between">
      <div class="flex flex-col items-start justify-start">
        <div class="">{t("user.change_password")}</div>
        <div class="text-muted-foreground text-sm">{t("user.change_password_desc")}</div>
      </div>
      <ChevronRight class="text-primary size-6" />
    </button>
    <button
      onclick={() => user_dialog?.open()}
      class="hover:bg-surface/50 flex flex-row items-center justify-between">
      <div class="flex flex-col items-start justify-start">
        <div class="">{t("user.change_info")}</div>
        <div class="text-muted-foreground text-sm">{t("user.change_info_desc")}</div>
      </div>
      <ChevronRight class="text-primary size-6" />
    </button>
    <button
      onclick={open_totp_dialog}
      class="hover:bg-surface/50 flex flex-row items-center justify-between">
      <div class="flex flex-col items-start justify-start">
        <div class=""><Trans k="user.setup_totp" /></div>
        <div class="text-muted-foreground text-sm">
          <Trans k="user.setup_totp_desc" />
        </div>
      </div>
      <ChevronRight class="text-primary size-6" />
    </button>
    <button
      onclick={() => logout_dialog?.open()}
      class="hover:bg-surface/50 flex flex-row items-center justify-between">
      <div class="flex flex-col items-start justify-start">
        <div class="">{t("user.logout")}</div>
        <div class="text-muted-foreground text-sm">
          {t("user.logout_desc")}
        </div>
      </div>
      <ChevronRight class="text-primary size-6" />
    </button>
  </div>
  {#if AUTH.user?.permissions.includes("management")}
    <Management />
  {/if}
</div>

<!-- ── Reset Password Dialog ── -->
<Dialog bind:this={password_dialog} onclose={reset_password_state}>
  <div class="text-primary-foreground text-lg">{t("user.password_dialog_title")}</div>
  <form class="flex flex-col" onsubmit={handle_change_password}>
    <input
      type="text"
      name="username"
      autocomplete="username"
      class="hidden"
      value={AUTH.user?.email} />
    <label for="old_password">{t("user.old_password")}</label>
    <input
      id="old_password"
      class="input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition"
      placeholder={t("user.old_password_placeholder")}
      autocomplete="current-password"
      type="password"
      bind:value={old_password} />
    <label for="new_password" class="mt-3">{t("user.new_password")}</label>
    <input
      autocomplete="new-password"
      id="new_password"
      class="input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition"
      placeholder={t("user.new_password_placeholder")}
      type="password"
      bind:value={new_password} />
    <label for="confirm_password" class="mt-3">{t("user.confirm_password")}</label>
    <input
      autocomplete="new-password"
      id="confirm_password"
      class="input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition"
      placeholder={t("user.confirm_password_placeholder")}
      type="password"
      bind:value={confirm_password} />
    <div class="text-muted-foreground mt-4">
      <Trans k="user.password_rule" />
    </div>
    {#if password_error}
      <div class="mt-2 text-sm text-red-500">{password_error}</div>
    {/if}
    <button
      type="submit"
      disabled={password_loading}
      class="bg-primary loading:bg-muted mt-4 flex flex-row items-center justify-center gap-2 rounded-lg px-3 py-2 text-white disabled:opacity-50 md:justify-start md:self-end">
      {#if password_loading}
        <LoaderCircle class="size-5 animate-spin" />
      {:else}
        <span>{t("common.submit")}</span>
      {/if}
    </button>
  </form>
</Dialog>

<!-- ── Update Account Info Dialog ── -->
<Dialog bind:this={user_dialog} onclose={reset_user_state}>
  <div class="text-primary-foreground text-lg">{t("user.info_dialog_title")}</div>
  <form class="flex flex-col" onsubmit={handle_update_user}>
    <label for="user_name">{t("user.nickname_label")}</label>
    <input
      id="user_name"
      class="input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition"
      placeholder={t("user.nickname_placeholder")}
      type="text"
      bind:value={user_name} />
    <label for="user_email" class="mt-3">{t("user.email_label")}</label>
    <input
      autocomplete="username"
      id="user_email"
      class="input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition"
      placeholder={t("user.email_placeholder")}
      type="email"
      bind:value={user_email} />
    {#if user_error}
      <div class="mt-2 text-sm text-red-500">{user_error}</div>
    {/if}
    <button
      type="submit"
      disabled={user_loading}
      class="bg-primary loading:bg-muted mt-4 flex flex-row items-center justify-center gap-2 rounded-lg px-3 py-2 text-white disabled:opacity-50 md:justify-start md:self-end">
      {#if user_loading}
        <LoaderCircle class="size-5 animate-spin" />
      {:else}
        <span>{t("common.submit")}</span>
      {/if}
    </button>
  </form>
</Dialog>

<!-- ── TOTP Dialog ── -->
<Dialog
  bind:this={totp_dialog}
  onclose={async () => {
    await tick();
    reset_totp_state;
  }}>
  <div class="text-primary-foreground text-lg"><Trans k="user.totp_dialog_title" /></div>
  {#if totp_step === "loading"}
    <div class="text-muted-foreground flex flex-col items-center gap-3 py-8">
      <LoaderCircle class="size-10 animate-spin" />
      <span>{t("user.generating_key")}</span>
    </div>
  {:else}
    {#if totp_error && !totp_secret}
      <div class="mt-2 text-sm text-red-500">{totp_error}</div>
    {/if}
    {#if totp_secret}
      <div>{t("user.scan_qr")}</div>

      {#if totp_qr_data_url}
        <div class="flex justify-center py-4">
          <img
            class="rounded-lg border p-2"
            src={totp_qr_data_url}
            alt={t("user.qr_alt")}
            width="256"
            height="256" />
        </div>
      {/if}

      <div class="flex flex-col gap-1">
        <label>{t("user.secret_label")}</label>
        <div class="flex flex-row items-center gap-2">
          <code
            class="bg-surface text-primary-foreground flex-1 overflow-x-auto rounded-lg border px-3 py-2 font-mono text-sm whitespace-nowrap"
            >{totp_secret}</code>
          <Tooltip.Root>
            <Tooltip.Trigger
              type="button"
              onclick={copy_totp_secret}
              class="bg-surface hover:bg-muted rounded-lg border p-2 transition-colors">
              {#if totp_copied}
                <Check class="size-5 text-green-500" />
              {:else}
                <Copy class="size-5" />
              {/if}
            </Tooltip.Trigger>
            <Tooltip.Content>{t("user.copy_secret")}</Tooltip.Content>
          </Tooltip.Root>
        </div>
      </div>

      <div class="mt-4 flex flex-col gap-2">
        <label for="totp_code">{t("user.code_label")}</label>
        <input
          id="totp_code"
          bind:this={totp_code_input}
          class="input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-center font-mono text-sm tracking-widest transition"
          placeholder={t("user.code_placeholder")}
          type="text"
          inputmode="numeric"
          maxlength={6}
          bind:value={totp_code} />
      </div>

      {#if totp_error}
        <div class="mt-2 text-sm text-red-500">{totp_error}</div>
      {/if}

      <div class="mt-4 flex flex-row items-center justify-end gap-2">
        <button
          type="button"
          disabled={totp_loading}
          onclick={() => handle_disable_totp()}
          class="text-muted-foreground hover:text-primary aph-tr px-3 py-2 text-sm underline underline-offset-2 disabled:opacity-50">
          <Trans k="user.disable_totp" />
        </button>
        <button
          type="button"
          disabled={totp_loading || !totp_code}
          onclick={handle_activate_totp}
          class="bg-primary loading:bg-muted flex flex-row items-center justify-center gap-2 rounded-lg px-3 py-2 text-white disabled:opacity-50">
          {#if totp_loading}
            <LoaderCircle class="size-5 animate-spin" />
          {:else}
            <span>{t("user.activate")}</span>
          {/if}
        </button>
      </div>
    {/if}
  {/if}
</Dialog>

<!-- ── Logout Dialog ── -->
<Dialog bind:this={logout_dialog}>
  <div class="text-primary-foreground text-lg">{t("user.logout_dialog_title")}</div>
  <div>{t("user.logout_confirm")}</div>
  <button
    type="button"
    onclick={handle_logout}
    class="bg-primary mt-4 flex flex-row items-center justify-center gap-2 rounded-lg px-3 py-2 text-white md:justify-start md:self-end">
    <span>{t("user.logout_button")}</span>
  </button>
</Dialog>
