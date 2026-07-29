<!-- @component
Management functionalities, conditionally shown in the User page.
-->
<script lang="ts">
  import { ChevronRight, Copy, Check, LoaderCircle } from "@lucide/svelte";
  import { OverlayScrollbarsComponent } from "overlayscrollbars-svelte";

  import Dialog from "@/components/Dialog.svelte";
  import { toast } from "@/components/toast.svelte";
  import {
    create_user,
    get_user_by_email,
    get_user_by_id,
    create_register_session,
    change_user_password,
    type User,
    type CreateUserResponse,
  } from "@/lib/api";
  import { AUTH } from "@/lib/auth.svelte";
  import { Checkbox } from "@/lib/components/ui/checkbox";
  import * as Select from "@/lib/components/ui/select";
  import * as Tooltip from "@/lib/components/ui/tooltip";
  import { t } from "@/lib/i18n.svelte";
  import Trans from "@/lib/Trans.svelte";
  import { transition_tick } from "@/lib/utils";

  // ── Create User Dialog ──
  let create_dialog: null | Dialog = $state(null);
  let create_email = $state("");
  let create_name = $state("");
  let create_is_management = $state(false);
  let create_loading = $state(false);
  let create_error = $state("");
  let create_result: CreateUserResponse | null = $state(null);

  function reset_create() {
    create_email = "";
    create_name = "";
    create_is_management = false;
    create_loading = false;
    create_error = "";
    create_result = null;
  }

  async function handle_create_user(e: SubmitEvent) {
    e.preventDefault();
    create_error = "";
    if (!AUTH.token) return;
    if (!create_email) {
      create_error = t("management.error_email_empty");
      return;
    }
    create_loading = true;
    const resp = await create_user(AUTH.token, {
      email: create_email,
      name: create_name || undefined,
      permissions: create_is_management ? (["management"] as "management"[]) : [],
    });
    create_loading = false;
    if (!resp.success) {
      create_error = resp.reason;
      return;
    }
    transition_tick(() => {
      create_result = resp.payload;
    });
    toast(t("toast.user_created"));
  }

  // ── Generate Registration Link Dialog ──
  let reglink_dialog: null | Dialog = $state(null);
  let reglink_expires = $state("1440"); // default 1 day
  const REGLINK_LABEL_KEYS: Record<string, string> = {
    "30": "management.minutes_30",
    "60": "management.hour_1",
    "360": "management.hours_6",
    "720": "management.hours_12",
    "1440": "management.day_1",
    "10080": "management.days_7",
  };
  let reglink_label = $derived(
    t(REGLINK_LABEL_KEYS[reglink_expires] ?? "management.select_expires"),
  );
  let reglink_loading = $state(false);
  let reglink_error = $state("");
  let reglink_token: string | null = $state(null);
  let reglink_url = $state("");
  let reglink_copied = $state(false);

  function reset_reglink() {
    reglink_expires = "1440";
    reglink_loading = false;
    reglink_error = "";
    reglink_token = null;
    reglink_url = "";
    reglink_copied = false;
  }

  async function handle_generate_reglink(e: SubmitEvent) {
    e.preventDefault();
    reglink_error = "";
    if (!AUTH.token) return;
    reglink_loading = true;
    const resp = await create_register_session(AUTH.token, Number(reglink_expires));
    reglink_loading = false;
    if (!resp.success) {
      reglink_error = resp.reason;
      return;
    }
    const token = resp.payload.token;
    const url = `${window.location.origin}/login#/register?token=${token}`;
    transition_tick(() => {
      reglink_token = token;
      reglink_url = url;
    });
    toast(t("toast.reglink_generated"));
  }

  async function copy_reglink() {
    if (!reglink_url) return;
    try {
      await navigator.clipboard.writeText(reglink_url);
      toast(t("common.copied"));
      reglink_copied = true;
      setTimeout(() => (reglink_copied = false), 1000);
    } catch (e) {
      toast(t("common.copy_failed"));
      console.error(e);
    }
  }

  // ── Query User Info Dialog ──
  let query_dialog: null | Dialog = $state(null);
  let query_input = $state("");
  let query_mode: "email" | "uuid" = $state("email");
  const QUERY_LABEL_KEYS: Record<string, string> = {
    email: "management.query_mode_email",
    uuid: "management.query_mode_uuid",
  };
  let query_label = $derived(t(QUERY_LABEL_KEYS[query_mode] ?? query_mode));
  let query_loading = $state(false);
  let query_error = $state("");
  let query_user: User | null = $state(null);

  function reset_query() {
    query_input = "";
    query_mode = "email";
    query_loading = false;
    query_error = "";
    query_user = null;
  }

  async function handle_query_user(e: SubmitEvent) {
    e.preventDefault();
    query_error = "";
    query_user = null;
    if (!AUTH.token || !query_input) return;
    query_loading = true;

    let resp;
    if (query_mode === "email") {
      resp = await get_user_by_email(AUTH.token, query_input);
    } else {
      resp = await get_user_by_id(AUTH.token, query_input);
    }
    query_loading = false;
    if (!resp.success) {
      query_error = resp.reason;
      return;
    }
    transition_tick(() => {
      query_user = resp.payload;
    });
  }

  // ── Change User Password Dialog ──
  let passwd_dialog: null | Dialog = $state(null);
  let passwd_identifier = $state(""); // email or UUID
  let passwd_loading = $state(false);
  let passwd_error = $state("");
  let passwd_new_password: string | null = $state(null);

  function reset_passwd() {
    passwd_identifier = "";
    passwd_loading = false;
    passwd_error = "";
    passwd_new_password = null;
  }

  function generate_random_password() {
    const chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()_-+=<>?";
    let password = "";
    for (let i = 0; i < 24; i++) {
      password += chars.charAt(Math.floor(Math.random() * chars.length));
    }
    return password;
  }

  async function handle_change_user_password(e: SubmitEvent) {
    e.preventDefault();
    passwd_error = "";
    passwd_new_password = null;
    if (!AUTH.token) return;

    passwd_loading = true;

    // Try to resolve identifier to UUID
    let target_id: string;
    const is_uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      passwd_identifier,
    );
    if (is_uuid) {
      target_id = passwd_identifier;
    } else {
      // Treat as email, look up UUID first
      const lookup = await get_user_by_email(AUTH.token, passwd_identifier);
      if (!lookup.success) {
        passwd_loading = false;
        passwd_error = t("management.error_user_not_found", { reason: lookup.reason });
        return;
      }
      target_id = lookup.payload.id;
    }

    const new_password = generate_random_password();
    const resp = await change_user_password(AUTH.token, target_id, new_password);
    passwd_loading = false;
    if (!resp.success) {
      passwd_error = resp.reason;
      return;
    }
    transition_tick(() => {
      passwd_new_password = new_password;
    });
  }
</script>

<!-- parent: flex flex-col gap-4 -->
<div class="text-primary-foreground border-b py-4 text-lg">
  <Trans k="management.title" />
</div>
<div class="-mt-4 flex flex-col divide-y *:p-4 *:focus:ring-0">
  <button
    onclick={() => create_dialog?.open()}
    class="hover:bg-surface/50 flex flex-row items-center justify-between">
    <div class="flex flex-col items-start justify-start">
      <div class="">{t("management.create_user")}</div>
      <div class="text-muted-foreground text-sm">
        {t("management.create_user_desc")}
      </div>
    </div>
    <ChevronRight class="text-primary size-6" />
  </button>
  <button
    onclick={() => reglink_dialog?.open()}
    class="hover:bg-surface/50 flex flex-row items-center justify-between">
    <div class="flex flex-col items-start justify-start">
      <div class="">{t("management.generate_reglink")}</div>
      <div class="text-muted-foreground text-sm">
        {t("management.generate_reglink_desc")}
      </div>
    </div>
    <ChevronRight class="text-primary size-6" />
  </button>
  <button
    onclick={() => query_dialog?.open()}
    class="hover:bg-surface/50 flex flex-row items-center justify-between">
    <div class="flex flex-col items-start justify-start">
      <div class="">{t("management.query_user")}</div>
      <div class="text-muted-foreground text-sm">{t("management.query_user_desc")}</div>
    </div>
    <ChevronRight class="text-primary size-6" />
  </button>
  <button
    onclick={() => passwd_dialog?.open()}
    class="hover:bg-surface/50 flex flex-row items-center justify-between">
    <div class="flex flex-col items-start justify-start">
      <div class="">{t("management.change_password")}</div>
      <div class="text-muted-foreground text-sm">
        <Trans k="management.change_password_desc" />
      </div>
    </div>
    <ChevronRight class="text-primary size-6" />
  </button>
</div>

<!-- ── Create User Dialog ── -->
<Dialog bind:this={create_dialog} class="flex flex-col" onclose={reset_create}>
  <div class="text-primary-foreground text-lg">{t("management.create_dialog_title")}</div>

  {#if create_result}
    <div style="view-transition-name: mgmt-create">
      <div class="mt-2 text-sm">{t("management.created_success")}</div>
      <div class="mt-3 flex flex-col gap-1">
        <label>{t("management.email_label")}</label>
        <div class="input-surface text-foreground rounded-lg border px-3 py-2 text-sm">
          {create_result.email}
        </div>
      </div>
      <div class="mt-3 flex flex-col gap-1">
        <label>{t("management.temp_password")}</label>
        <OverlayScrollbarsComponent
          role="button"
          onclick={async () => {
            await navigator.clipboard.writeText(create_result!.password);
            toast(t("toast.manager_password_copied"));
          }}
          class="aph input-surface text-foreground flex cursor-pointer items-center justify-between overflow-x-scroll rounded-lg border px-3 py-2 font-mono text-sm"
          options={{ scrollbars: { autoHide: "leave" }, overflow: { y: "hidden" } }}>
          <span>{create_result.password}</span>
        </OverlayScrollbarsComponent>
      </div>
      <button
        type="button"
        onclick={() => {
          reset_create();
          create_dialog?.close();
        }}
        class="bg-primary mt-4 flex items-center justify-center rounded-lg px-3 py-2 text-white md:self-end">
        {t("common.close")}
      </button>
    </div>
  {:else}
    <form
      style="view-transition-name: mgmt-create"
      class="mt-2 flex flex-col"
      onsubmit={handle_create_user}>
      <label for="create-email">{t("management.email_label")}</label>
      <input
        id="create-email"
        class="input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition"
        placeholder="user@example.com"
        type="email"
        bind:value={create_email} />

      <label for="create-name" class="mt-3">{t("common.nickname")}</label>
      <input
        id="create-name"
        class="input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition"
        placeholder={t("management.nickname_placeholder")}
        type="text"
        bind:value={create_name} />

      <label class="mt-3">{t("management.permissions_label")}</label>
      <div class="mt-1 flex flex-row items-center gap-2">
        <Checkbox id="create-perm-mgmt" bind:checked={create_is_management} />
        <label for="create-perm-mgmt" class="text-sm">{t("management.admin")}</label>
      </div>

      {#if create_error}
        <div class="mt-2 text-sm text-red-500">{create_error}</div>
      {/if}

      <button
        type="submit"
        disabled={create_loading}
        class="bg-primary loading:bg-muted mt-4 flex items-center justify-center gap-2 rounded-lg px-3 py-2 text-white disabled:opacity-50 md:self-end">
        {#if create_loading}
          <LoaderCircle class="size-5 animate-spin" />
        {:else}
          <span>{t("common.create")}</span>
        {/if}
      </button>
    </form>
  {/if}
</Dialog>

<!-- ── Generate Registration Link Dialog ── -->
<Dialog class="flex flex-col" bind:this={reglink_dialog} onclose={reset_reglink}>
  <div class="text-primary-foreground text-lg">{t("management.reglink_dialog_title")}</div>

  {#if reglink_token}
    <div style="view-transition-name: mgmt-reglink" class="flex flex-col">
      <div class="mt-3 flex flex-col gap-1">
        <label>{t("management.reglink_label")}</label>
        <div
          class="input-surface text-foreground flex items-center justify-between gap-2 rounded-lg border px-3 py-2 text-sm break-all">
          <code class="flex-1">{reglink_url}</code>
          <Tooltip.Root>
            <Tooltip.Trigger
              type="button"
              onclick={copy_reglink}
              class="hover:bg-muted shrink-0 rounded p-1 transition-colors">
              {#if reglink_copied}
                <Check class="size-5 text-green-500" />
              {:else}
                <Copy class="size-5" />
              {/if}
            </Tooltip.Trigger>
            <Tooltip.Content>{t("management.copy_link")}</Tooltip.Content>
          </Tooltip.Root>
        </div>
      </div>
      <div class="text-muted-foreground mt-2 text-xs">{t("management.reglink_single_use")}</div>
      <button
        type="button"
        onclick={() => {
          reset_reglink();
          reglink_dialog?.close();
        }}
        class="bg-primary mt-4 flex items-center justify-center rounded-lg px-3 py-2 text-white md:self-end">
        {t("common.close")}
      </button>
    </div>
  {:else}
    <form
      style="view-transition-name: mgmt-reglink"
      class="mt-2 flex flex-col"
      onsubmit={handle_generate_reglink}>
      <label for="reglink-expires">{t("management.expires_label")}</label>
      <Select.Root bind:value={reglink_expires}>
        <Select.Trigger class="mt-1 w-full" id="reglink-expires">
          {reglink_label}
        </Select.Trigger>
        <Select.Content>
          <Select.Item value="30" label={t("management.minutes_30")}
            >{t("management.minutes_30")}</Select.Item>
          <Select.Item value="60" label={t("management.hour_1")}
            >{t("management.hour_1")}</Select.Item>
          <Select.Item value="360" label={t("management.hours_6")}
            >{t("management.hours_6")}</Select.Item>
          <Select.Item value="720" label={t("management.hours_12")}
            >{t("management.hours_12")}</Select.Item>
          <Select.Item value="1440" label={t("management.day_1")}
            >{t("management.day_1")}</Select.Item>
          <Select.Item value="10080" label={t("management.days_7")}
            >{t("management.days_7")}</Select.Item>
        </Select.Content>
      </Select.Root>

      {#if reglink_error}
        <div class="mt-2 text-sm text-red-500">{reglink_error}</div>
      {/if}

      <button
        type="submit"
        disabled={reglink_loading}
        class="bg-primary loading:bg-muted mt-4 flex items-center justify-center gap-2 rounded-lg px-3 py-2 text-white disabled:opacity-50 md:self-end">
        {#if reglink_loading}
          <LoaderCircle class="size-5 animate-spin" />
        {:else}
          <span>{t("common.submit")}</span>
        {/if}
      </button>
    </form>
  {/if}
</Dialog>

<!-- ── Query User Info Dialog ── -->
<Dialog bind:this={query_dialog} class="flex flex-col" onclose={reset_query}>
  <div class="text-primary-foreground text-lg">{t("management.query_dialog_title")}</div>
  <form class="mt-2 flex flex-col" onsubmit={handle_query_user}>
    <label for="query-input"><Trans k="management.query_input_label" /></label>
    <div class="mt-1 flex flex-row gap-2">
      <input
        id="query-input"
        class="input-surface block flex-1 rounded-lg border px-3 py-1 text-sm transition"
        type="text"
        bind:value={query_input} />
      <Select.Root bind:value={query_mode}>
        <Select.Trigger class="w-24">
          {query_label}
        </Select.Trigger>
        <Select.Content>
          <Select.Item value="email" label={t("management.query_mode_email")}
            >{t("management.query_mode_email")}</Select.Item>
          <Select.Item value="uuid" label={t("management.query_mode_uuid")}
            >{t("management.query_mode_uuid")}</Select.Item>
        </Select.Content>
      </Select.Root>
    </div>

    {#if query_error}
      <div class="mt-2 text-sm text-red-500">{query_error}</div>
    {/if}
    {#if query_user}
      <div
        style="view-transition-name: mgmt-query-result"
        class="*:odd:text-muted-foreground mt-4 grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 rounded-lg border p-3 text-sm *:py-1">
        <span>{t("common.uuid")}</span><code class="font-mono">{query_user.id}</code>
        <span>{t("management.query_result_nickname")}</span><span>{query_user.name}</span>
        <span>{t("management.query_result_email")}</span><span>{query_user.email}</span>
        <span>{t("management.query_result_permissions")}</span><span
          >{query_user.permissions.length
            ? query_user.permissions.join(", ")
            : t("management.query_result_none")}</span>
      </div>
    {/if}

    <button
      type="submit"
      disabled={query_loading || !query_input}
      class="bg-primary loading:bg-muted mt-4 flex items-center justify-center gap-2 rounded-lg px-3 py-2 text-white disabled:opacity-50 md:self-end">
      {#if query_loading}
        <LoaderCircle class="size-5 animate-spin" />
      {:else}
        <span>{t("common.submit")}</span>
      {/if}
    </button>
  </form>
</Dialog>

<!-- ── Change User Password Dialog ── -->
<Dialog bind:this={passwd_dialog} class="flex flex-col" onclose={reset_passwd}>
  <div class="text-primary-foreground text-lg">{t("management.password_dialog_title")}</div>

  {#if passwd_new_password}
    <div style="view-transition-name: mgmt-passwd">
      <div class="text-primary my-1 text-sm">{t("management.password_updated")}</div>
      <div class="flex flex-col gap-1">
        <label>{t("management.new_password_label")}</label>
        <div
          class="input-surface text-foreground flex items-center justify-between rounded-lg border px-3 py-2 font-mono text-sm">
          <span>{passwd_new_password}</span>
          <Tooltip.Root>
            <Tooltip.Trigger
              type="button"
              onclick={async () => {
                try {
                  await navigator.clipboard.writeText(passwd_new_password!);
                  toast(t("common.copied"));
                } catch {
                  toast(t("common.copy_failed"));
                }
              }}
              class="hover:bg-muted shrink-0 rounded p-1 transition-colors">
              <Copy class="size-4" />
            </Tooltip.Trigger>
            <Tooltip.Content>{t("management.copy_password")}</Tooltip.Content>
          </Tooltip.Root>
        </div>
      </div>
      <div class="text-muted-foreground mt-1 text-xs">
        {t("management.password_note")}
      </div>
      <button
        type="button"
        onclick={() => {
          reset_passwd();
          passwd_dialog?.close();
        }}
        class="bg-primary mt-2 flex items-center justify-center rounded-lg px-3 py-2 text-white md:self-end">
        {t("common.close")}
      </button>
    </div>
  {:else}
    <form
      style="view-transition-name: mgmt-passwd"
      class="mt-2 flex flex-col"
      onsubmit={handle_change_user_password}>
      <label for="passwd-identifier">{t("management.user_email_label")}</label>
      <input
        id="passwd-identifier"
        class="input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition"
        placeholder="user@example.com"
        type="text"
        bind:value={passwd_identifier} />

      {#if passwd_error}
        <div class="mt-2 text-sm text-red-500">{passwd_error}</div>
      {/if}

      <button
        type="submit"
        disabled={passwd_loading || !passwd_identifier}
        class="bg-primary loading:bg-muted mt-4 flex items-center justify-center gap-2 rounded-lg px-3 py-2 text-white disabled:opacity-50 md:self-end">
        {#if passwd_loading}
          <LoaderCircle class="size-5 animate-spin" />
        {:else}
          <span>{t("management.reset_password")}</span>
        {/if}
      </button>
    </form>
  {/if}
</Dialog>
