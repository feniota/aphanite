<!-- @component
Management functionalities, conditionally shown in the User page.
-->
<script lang="ts">
  import { ChevronRight, Copy, Check, LoaderCircle } from "@lucide/svelte";
  import { OverlayScrollbarsComponent } from "overlayscrollbars-svelte";

  import Dialog from "@/components/Dialog.svelte";
  import Space from "@/components/Space.svelte";
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
      create_error = "邮箱不能为空";
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
    toast("用户创建成功");
  }

  // ── Generate Registration Link Dialog ──
  let reglink_dialog: null | Dialog = $state(null);
  let reglink_expires = $state("1440"); // default 1 day
  const reglink_labels: Record<string, string> = {
    "30": "30 分钟",
    "60": "1 小时",
    "360": "6 小时",
    "720": "12 小时",
    "1440": "1 天",
    "10080": "7 天",
  };
  let reglink_label = $derived(reglink_labels[reglink_expires] ?? "选择过期时间");
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
    toast("注册链接已生成");
  }

  async function copy_reglink() {
    if (!reglink_url) return;
    try {
      await navigator.clipboard.writeText(reglink_url);
      toast("已复制到剪贴板");
      reglink_copied = true;
      setTimeout(() => (reglink_copied = false), 1000);
    } catch (e) {
      toast("复制失败");
      console.error(e);
    }
  }

  // ── Query User Info Dialog ──
  let query_dialog: null | Dialog = $state(null);
  let query_input = $state("");
  let query_mode: "email" | "uuid" = $state("email");
  const query_labels: Record<string, string> = { email: "邮箱", uuid: "UUID" };
  let query_label = $derived(query_labels[query_mode] ?? query_mode);
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
        passwd_error = "未找到该用户：" + lookup.reason;
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
  管理此<Space />Aphanite<Space />实例
</div>
<div class="-mt-4 flex flex-col divide-y *:p-4 *:focus:ring-0">
  <button
    onclick={() => create_dialog?.open()}
    class="hover:bg-surface/50 flex flex-row items-center justify-between">
    <div class="flex flex-col items-start justify-start">
      <div class="">创建用户</div>
      <div class="text-muted-foreground text-sm">
        在面板内直接创建新用户。如果您要为他人注册账号，生成注册链接是更好的选择。
      </div>
    </div>
    <ChevronRight class="text-primary size-6" />
  </button>
  <button
    onclick={() => reglink_dialog?.open()}
    class="hover:bg-surface/50 flex flex-row items-center justify-between">
    <div class="flex flex-col items-start justify-start">
      <div class="">生成注册链接</div>
      <div class="text-muted-foreground text-sm">
        生成一串链接，他人可直接用来注册，即使实例是私密的也没有关系。
      </div>
    </div>
    <ChevronRight class="text-primary size-6" />
  </button>
  <button
    onclick={() => query_dialog?.open()}
    class="hover:bg-surface/50 flex flex-row items-center justify-between">
    <div class="flex flex-col items-start justify-start">
      <div class="">查询用户信息</div>
      <div class="text-muted-foreground text-sm">查询指定用户的信息。</div>
    </div>
    <ChevronRight class="text-primary size-6" />
  </button>
  <button
    onclick={() => passwd_dialog?.open()}
    class="hover:bg-surface/50 flex flex-row items-center justify-between">
    <div class="flex flex-col items-start justify-start">
      <div class="">修改用户密码</div>
      <div class="text-muted-foreground text-sm">
        如果用户彻底忘记密码了，您可以给<Space />TA<Space />生成一个新密码。
      </div>
    </div>
    <ChevronRight class="text-primary size-6" />
  </button>
</div>

<!-- ── Create User Dialog ── -->
<Dialog bind:this={create_dialog} class="flex flex-col" onclose={reset_create}>
  <div class="text-primary-foreground text-lg">创建用户</div>

  {#if create_result}
    <div style="view-transition-name: mgmt-create">
      <div class="mt-2 text-sm">用户已创建！请务必将以下密码交给新用户。</div>
      <div class="mt-3 flex flex-col gap-1">
        <label>邮箱</label>
        <div class="input-surface text-foreground rounded-lg border px-3 py-2 text-sm">
          {create_result.email}
        </div>
      </div>
      <div class="mt-3 flex flex-col gap-1">
        <label>临时密码</label>
        <OverlayScrollbarsComponent
          role="button"
          onclick={async () => {
            await navigator.clipboard.writeText(create_result!.password);
            toast("密码已复制到剪贴板");
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
        关闭
      </button>
    </div>
  {:else}
    <form
      style="view-transition-name: mgmt-create"
      class="mt-2 flex flex-col"
      onsubmit={handle_create_user}>
      <label for="create-email">邮箱</label>
      <input
        id="create-email"
        class="input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition"
        placeholder="user@example.com"
        type="email"
        bind:value={create_email} />

      <label for="create-name" class="mt-3">昵称</label>
      <input
        id="create-name"
        class="input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition"
        placeholder="留空则使用邮箱"
        type="text"
        bind:value={create_name} />

      <label class="mt-3">权限</label>
      <div class="mt-1 flex flex-row items-center gap-2">
        <Checkbox id="create-perm-mgmt" bind:checked={create_is_management} />
        <label for="create-perm-mgmt" class="text-sm">管理员</label>
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
          <span>创建</span>
        {/if}
      </button>
    </form>
  {/if}
</Dialog>

<!-- ── Generate Registration Link Dialog ── -->
<Dialog class="flex flex-col" bind:this={reglink_dialog} onclose={reset_reglink}>
  <div class="text-primary-foreground text-lg">生成注册链接</div>

  {#if reglink_token}
    <div style="view-transition-name: mgmt-reglink" class="flex flex-col">
      <div class="mt-3 flex flex-col gap-1">
        <label>注册链接</label>
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
            <Tooltip.Content>复制链接</Tooltip.Content>
          </Tooltip.Root>
        </div>
      </div>
      <div class="text-muted-foreground mt-2 text-xs">该链接只能使用一次。</div>
      <button
        type="button"
        onclick={() => {
          reset_reglink();
          reglink_dialog?.close();
        }}
        class="bg-primary mt-4 flex items-center justify-center rounded-lg px-3 py-2 text-white md:self-end">
        关闭
      </button>
    </div>
  {:else}
    <form
      style="view-transition-name: mgmt-reglink"
      class="mt-2 flex flex-col"
      onsubmit={handle_generate_reglink}>
      <label for="reglink-expires">过期时间</label>
      <Select.Root bind:value={reglink_expires}>
        <Select.Trigger class="mt-1 w-full" id="reglink-expires">
          {reglink_label}
        </Select.Trigger>
        <Select.Content>
          <Select.Item value="30" label="30 分钟">30 分钟</Select.Item>
          <Select.Item value="60" label="1 小时">1 小时</Select.Item>
          <Select.Item value="360" label="6 小时">6 小时</Select.Item>
          <Select.Item value="720" label="12 小时">12 小时</Select.Item>
          <Select.Item value="1440" label="1 天">1 天</Select.Item>
          <Select.Item value="10080" label="7 天">7 天</Select.Item>
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
          <span>生成</span>
        {/if}
      </button>
    </form>
  {/if}
</Dialog>

<!-- ── Query User Info Dialog ── -->
<Dialog bind:this={query_dialog} class="flex flex-col" onclose={reset_query}>
  <div class="text-primary-foreground text-lg">查询用户信息</div>
  <form class="mt-2 flex flex-col" onsubmit={handle_query_user}>
    <label for="query-input">邮箱或 UUID</label>
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
          <Select.Item value="email" label="邮箱">邮箱</Select.Item>
          <Select.Item value="uuid" label="UUID">UUID</Select.Item>
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
        <span>UUID</span><code class="font-mono">{query_user.id}</code>
        <span>昵称</span><span>{query_user.name}</span>
        <span>邮箱</span><span>{query_user.email}</span>
        <span>权限</span><span
          >{query_user.permissions.length ? query_user.permissions.join(", ") : "无"}</span>
      </div>
    {/if}

    <button
      type="submit"
      disabled={query_loading || !query_input}
      class="bg-primary loading:bg-muted mt-4 flex items-center justify-center gap-2 rounded-lg px-3 py-2 text-white disabled:opacity-50 md:self-end">
      {#if query_loading}
        <LoaderCircle class="size-5 animate-spin" />
      {:else}
        <span>查询</span>
      {/if}
    </button>
  </form>
</Dialog>

<!-- ── Change User Password Dialog ── -->
<Dialog bind:this={passwd_dialog} class="flex flex-col" onclose={reset_passwd}>
  <div class="text-primary-foreground text-lg">修改用户密码</div>

  {#if passwd_new_password}
    <div style="view-transition-name: mgmt-passwd">
      <div class="text-primary my-1 text-sm">密码已更新。</div>
      <div class="flex flex-col gap-1">
        <label>新密码</label>
        <div
          class="input-surface text-foreground flex items-center justify-between rounded-lg border px-3 py-2 font-mono text-sm">
          <span>{passwd_new_password}</span>
          <Tooltip.Root>
            <Tooltip.Trigger
              type="button"
              onclick={async () => {
                try {
                  await navigator.clipboard.writeText(passwd_new_password!);
                  toast("已复制到剪贴板");
                } catch {
                  toast("复制失败");
                }
              }}
              class="hover:bg-muted shrink-0 rounded p-1 transition-colors">
              <Copy class="size-4" />
            </Tooltip.Trigger>
            <Tooltip.Content>复制密码</Tooltip.Content>
          </Tooltip.Root>
        </div>
      </div>
      <div class="text-muted-foreground mt-1 text-xs">
        该密码为系统随机生成，请提醒用户尽快登录并修改密码。
      </div>
      <button
        type="button"
        onclick={() => {
          reset_passwd();
          passwd_dialog?.close();
        }}
        class="bg-primary mt-2 flex items-center justify-center rounded-lg px-3 py-2 text-white md:self-end">
        关闭
      </button>
    </div>
  {:else}
    <form
      style="view-transition-name: mgmt-passwd"
      class="mt-2 flex flex-col"
      onsubmit={handle_change_user_password}>
      <label for="passwd-identifier">用户邮箱</label>
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
          <span>重置密码</span>
        {/if}
      </button>
    </form>
  {/if}
</Dialog>
