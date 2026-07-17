<script lang="ts">
  import QRCode from "qrcode";

  import {
    get_me,
    update_me,
    change_password,
    issue_totp,
    activate_totp,
    delete_totp,
    create_verification,
    complete_verification,
    ApiError,
  } from "../lib/api";
  import type { User } from "../lib/api";
  import { AUTH } from "../lib/auth.svelte";
  import { show } from "../lib/toast.svelte";

  let user = $state<User | null>(null);
  let loading = $state(true);

  // 编辑个人信息
  let edit_name = $state("");
  let edit_email = $state("");
  let editing = $state(false);
  let save_loading = $state(false);

  // 修改密码弹窗
  let pwd_modal = $state(false);
  let pwd_step = $state(1);
  let pwd_method = $state<"password" | "totp">("password");
  let pwd_old_password = $state("");
  let pwd_otp_code = $state("");
  let pwd_new_password = $state("");
  let pwd_confirm = $state("");
  let pwd_loading = $state(false);
  let pwd_error = $state("");

  // TOTP
  let totp_enabled = $state(false);
  let totp_modal = $state(false);
  let totp_secret = $state("");
  let totp_url = $state("");
  let totp_code = $state("");
  let totp_loading = $state(false);
  let totp_error = $state("");
  let totp_qr_loaded = $state(false);
  let totp_copied = $state(false);
  let qr_svg = $state("");
  let disable_loading = $state(false);
  let totp_shake = $state(false);

  $effect(() => {
    if (totp_url) {
      totp_qr_loaded = false;
      QRCode.toString(totp_url, {
        type: "svg",
        color: { dark: "#000", light: "#fff" },
        width: 432,
      }).then(svg => {
        qr_svg = svg;
        totp_qr_loaded = true;
      });
    }
  });

  function copy_secret() {
    navigator.clipboard.writeText(totp_secret);
    totp_copied = true;
    setTimeout(() => (totp_copied = false), 2000);
  }

  $effect(() => {
    if (!AUTH.token) return;
    get_me(AUTH.token)
      .then(u => {
        user = u;
        AUTH.set_session(AUTH.token!, u);
        // 检查 TOTP 是否已启用
        return create_verification(u.email, "totp").then(() => (totp_enabled = true));
      })
      .catch(() => {})
      .finally(() => (loading = false));
  });

  function start_edit() {
    if (!user) return;
    edit_name = user.name;
    edit_email = user.email;
    editing = true;
  }

  function cancel_edit() {
    editing = false;
  }

  async function save_profile(e: SubmitEvent) {
    e.preventDefault();
    if (!AUTH.token) return;

    // 前端校验昵称
    if (edit_name) {
      const nameLen = [...edit_name].length;
      if (nameLen < 3 || nameLen > 16) {
        show(`昵称长度需为 3–16 个字符，当前为 ${nameLen} 个字符`);
        return;
      }
      if (!/^[a-zA-Z0-9_-]+$/.test(edit_name)) {
        show("昵称只能包含字母、数字、下划线和连字符（-）");
        return;
      }
    }

    save_loading = true;
    try {
      const updated = await update_me(AUTH.token, {
        name: edit_name || undefined,
        email: edit_email,
      });
      user = updated;
      AUTH.set_session(AUTH.token!, updated);
      editing = false;
      show("保存成功");
    } catch (err) {
      if (err instanceof ApiError && err.status === 422) {
        show("昵称或邮箱格式不正确，请检查后重试");
      } else {
        show("保存失败，请重试");
      }
    } finally {
      save_loading = false;
    }
  }

  function open_pwd_modal() {
    pwd_step = 1;
    pwd_method = totp_enabled ? "totp" : "password";
    pwd_old_password = "";
    pwd_otp_code = "";
    pwd_new_password = "";
    pwd_confirm = "";
    pwd_error = "";
    pwd_modal = true;
  }

  function close_pwd_modal() {
    pwd_modal = false;
  }

  function next_pwd_step() {
    pwd_error = "";
    pwd_step = 2;
  }

  async function submit_password_change() {
    if (!AUTH.token) return;
    if (pwd_method === "password" && !pwd_old_password) {
      pwd_error = "请输入旧密码";
      return;
    }
    if (pwd_method === "totp" && pwd_otp_code.length !== 6) {
      pwd_error = "请输入 6 位 TOTP 验证码";
      return;
    }
    if (pwd_new_password !== pwd_confirm) {
      pwd_error = "两次输入的密码不一致";
      return;
    }
    if (pwd_new_password.length < 8) {
      pwd_error = `密码长度不能少于 8 个字符，当前为 ${pwd_new_password.length} 个字符`;
      return;
    }
    if (pwd_new_password.length > 128) {
      pwd_error = `密码长度不能超过 128 个字符，当前为 ${pwd_new_password.length} 个字符`;
      return;
    }
    pwd_error = "";
    pwd_loading = true;
    try {
      let otp_token: string | undefined;
      if (pwd_method === "totp") {
        const { id } = await create_verification(user!.email, "totp");
        const res = await complete_verification(id, pwd_otp_code);
        otp_token = res.otp_token;
      }
      await change_password(AUTH.token, {
        old_password: pwd_method === "password" ? pwd_old_password : undefined,
        otp_token,
        new_password: pwd_new_password,
      });
      show("密码修改成功");
      close_pwd_modal();
    } catch (err) {
      if (err instanceof ApiError && err.status === 422) {
        pwd_error = "密码格式不正确，请检查后重试";
      } else {
        show("修改失败，请重试");
      }
    } finally {
      pwd_loading = false;
    }
  }

  // ── TOTP ──

  async function open_totp_modal() {
    if (!AUTH.token) return;
    totp_error = "";
    totp_copied = false;
    totp_qr_loaded = false;
    totp_code = "";
    totp_loading = true;
    totp_modal = true;
    try {
      const res = await issue_totp(AUTH.token);
      totp_secret = res.secret;
      totp_url = res.otpauth_url;
    } catch (err) {
      totp_error = "请求失败，请重试";
    } finally {
      totp_loading = false;
    }
  }

  async function cancel_totp_setup() {
    totp_modal = false;
    // 取消时删除刚刚签发的 TOTP 密钥
    if (AUTH.token) {
      try {
        await delete_totp(AUTH.token);
      } catch {
        /* ignore */
      }
    }
  }

  async function submit_totp_setup() {
    if (!AUTH.token || !user) return;
    totp_error = "";
    if (totp_code.length !== 6) {
      totp_shake = true;
      setTimeout(() => (totp_shake = false), 500);
      return;
    }
    totp_loading = true;
    try {
      const { id } = await create_verification(user.email, "totp");
      const { otp_token } = await complete_verification(id, totp_code);
      await activate_totp(AUTH.token, otp_token);
      totp_enabled = true;
      totp_modal = false;
    } catch {
      totp_error = "验证码错误，请重试";
      totp_shake = true;
      setTimeout(() => (totp_shake = false), 500);
    } finally {
      totp_loading = false;
    }
  }

  async function disable_totp() {
    if (!AUTH.token) return;
    disable_loading = true;
    try {
      await delete_totp(AUTH.token);
      totp_enabled = false;
    } catch {
      /* ignore */
    } finally {
      disable_loading = false;
    }
  }
</script>

<div class="mx-auto max-w-2xl p-8">
  <h1 class="text-2xl font-bold">个人资料</h1>

  {#if loading}
    <p class="text-muted-foreground mt-4 text-sm">加载中…</p>
  {:else if user}
    <!-- 基本信息 -->
    <div
      class="mt-6 rounded-xl border border-slate-200 bg-white p-6 dark:border-slate-700 dark:bg-slate-800">
      <h2 class="text-lg font-semibold text-slate-900 dark:text-slate-100">基本信息</h2>

      {#if editing}
        <form class="mt-4 space-y-4" onsubmit={save_profile}>
          <div>
            <label class="block text-sm font-medium text-slate-700 dark:text-slate-300"
              >用户名</label>
            <input
              type="text"
              bind:value={edit_name}
              class="mt-1 block w-full max-w-xs rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100" />
          </div>
          <div>
            <label class="block text-sm font-medium text-slate-700 dark:text-slate-300">邮箱</label>
            <input
              type="email"
              bind:value={edit_email}
              required
              class="mt-1 block w-full max-w-xs rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100" />
          </div>
          <div class="flex gap-2">
            <button
              type="submit"
              disabled={save_loading}
              class="cursor-pointer rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white hover:bg-indigo-700 disabled:opacity-60 dark:bg-indigo-500 dark:hover:bg-indigo-600">
              {save_loading ? "保存中…" : "保存"}
            </button>
            <button
              type="button"
              onclick={cancel_edit}
              disabled={save_loading}
              class="cursor-pointer rounded-lg border border-slate-300 px-4 py-2 text-sm text-slate-600 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-400 dark:hover:bg-slate-700">
              取消
            </button>
          </div>
        </form>
      {:else}
        <dl class="mt-4 space-y-3 text-sm">
          <div class="flex gap-2">
            <dt class="w-16 text-slate-500 dark:text-slate-400">ID</dt>
            <dd class="font-mono text-slate-700 dark:text-slate-300">{user.id.slice(0, 8)}…</dd>
          </div>
          <div class="flex gap-2">
            <dt class="w-16 text-slate-500 dark:text-slate-400">用户名</dt>
            <dd class="text-slate-900 dark:text-slate-100">{user.name}</dd>
          </div>
          <div class="flex gap-2">
            <dt class="w-16 text-slate-500 dark:text-slate-400">邮箱</dt>
            <dd class="text-slate-900 dark:text-slate-100">{user.email}</dd>
          </div>
          {#if user.permissions.length > 0}
            <div class="flex gap-2">
              <dt class="w-16 text-slate-500 dark:text-slate-400">权限</dt>
              <dd class="text-slate-900 dark:text-slate-100">
                {#if user.permissions.includes("management")}
                  <span
                    class="rounded bg-amber-100 px-1.5 py-0.5 text-xs font-medium text-amber-800 dark:bg-amber-900 dark:text-amber-300"
                    >管理员</span>
                {/if}
              </dd>
            </div>
          {/if}
        </dl>

        <button
          onclick={start_edit}
          class="mt-4 cursor-pointer rounded-lg border border-slate-300 px-4 py-2 text-sm text-slate-600 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-400 dark:hover:bg-slate-700">
          编辑
        </button>
      {/if}
    </div>

    <!-- 认证信息 -->
    <div
      class="mt-6 rounded-xl border border-slate-200 bg-white p-6 dark:border-slate-700 dark:bg-slate-800">
      <h2 class="text-lg font-semibold text-slate-900 dark:text-slate-100">认证信息</h2>

      <!-- 修改密码 -->
      <div class="mt-4 border-b border-slate-100 pb-6 dark:border-slate-700">
        <div class="flex items-center justify-between">
          <h3 class="font-medium text-slate-700 dark:text-slate-300">密码</h3>
          <button
            onclick={open_pwd_modal}
            class="cursor-pointer rounded-lg border border-slate-300 px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-400 dark:hover:bg-slate-700">
            修改密码
          </button>
        </div>
      </div>

      <!-- 修改密码弹窗 -->
      {#if pwd_modal}
        <div
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
          onclick={close_pwd_modal}>
          <div
            class="w-full max-w-md rounded-xl bg-white p-6 shadow-xl dark:bg-slate-800"
            onclick={e => e.stopPropagation()}>
            <h3 class="text-lg font-semibold text-slate-900 dark:text-slate-100">修改密码</h3>

            {#if pwd_step === 1}
              <p class="mt-4 text-sm text-slate-500 dark:text-slate-400">选择验证方式</p>
              <div class="mt-3 space-y-3">
                <button
                  onclick={() => {
                    pwd_method = "password";
                    next_pwd_step();
                  }}
                  class="w-full cursor-pointer rounded-xl border-2 border-slate-200 p-4 text-left transition hover:border-slate-300 dark:border-slate-600 dark:hover:border-slate-500">
                  <span class="text-sm font-medium text-slate-900 dark:text-slate-100">旧密码</span>
                  <span class="mt-1 block text-xs text-slate-500 dark:text-slate-400"
                    >使用当前密码验证身份</span>
                </button>
                <button
                  onclick={() => {
                    pwd_method = "totp";
                    next_pwd_step();
                  }}
                  disabled={!totp_enabled}
                  class="w-full cursor-pointer rounded-xl border-2 border-slate-200 p-4 text-left transition hover:border-slate-300 disabled:opacity-40 dark:border-slate-600 dark:hover:border-slate-500">
                  <span class="text-sm font-medium text-slate-900 dark:text-slate-100">TOTP</span>
                  <span class="mt-1 block text-xs text-slate-500 dark:text-slate-400"
                    >使用认证器应用生成的验证码</span>
                </button>
              </div>
              <div class="mt-6 flex justify-end">
                <button
                  onclick={close_pwd_modal}
                  class="cursor-pointer rounded-lg border border-slate-300 px-4 py-2 text-sm text-slate-600 dark:border-slate-600 dark:text-slate-400"
                  >取消</button>
              </div>
            {:else if pwd_step === 2}
              <p class="mt-4 text-sm text-slate-500 dark:text-slate-400">
                验证：{pwd_method === "password" ? "旧密码" : "TOTP"}
              </p>
              {#if pwd_method === "password"}
                <input
                  type="password"
                  bind:value={pwd_old_password}
                  placeholder="当前密码"
                  class="mt-2 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100" />
              {:else}
                <input
                  type="text"
                  bind:value={pwd_otp_code}
                  maxlength="6"
                  placeholder="6 位 TOTP 验证码"
                  class="mt-2 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100" />
              {/if}
              <p class="mt-4 text-sm text-slate-500 dark:text-slate-400">设置新密码</p>
              <input
                type="password"
                bind:value={pwd_new_password}
                placeholder="新密码"
                class="mt-2 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100" />
              <input
                type="password"
                bind:value={pwd_confirm}
                placeholder="确认新密码"
                class="mt-2 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100" />
              {#if pwd_error}
                <p class="mt-2 text-sm text-red-600 dark:text-red-400">{pwd_error}</p>
              {/if}
              <div class="mt-6 flex justify-between">
                <button
                  onclick={() => (pwd_step = 1)}
                  disabled={pwd_loading}
                  class="cursor-pointer rounded-lg border border-slate-300 px-4 py-2 text-sm text-slate-600 dark:border-slate-600 dark:text-slate-400"
                  >← 上一步</button>
                <button
                  onclick={submit_password_change}
                  disabled={pwd_loading}
                  class="cursor-pointer rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white hover:bg-indigo-700 disabled:opacity-60 dark:bg-indigo-500 dark:hover:bg-indigo-600"
                  >{pwd_loading ? "…" : "确认修改"}</button>
              </div>
            {/if}
          </div>
        </div>
      {/if}
      <div class="pt-6">
        <div class="flex items-center justify-between">
          <h3 class="font-medium text-slate-700 dark:text-slate-300">TOTP</h3>
          {#if totp_enabled}
            <button
              onclick={disable_totp}
              disabled={disable_loading}
              class="cursor-pointer rounded-lg border border-red-300 px-3 py-1.5 text-sm text-red-600 hover:bg-red-50 disabled:opacity-60 dark:border-red-700 dark:text-red-400 dark:hover:bg-red-950">
              {disable_loading ? "…" : "禁用"}
            </button>
          {:else}
            <button
              onclick={open_totp_modal}
              class="cursor-pointer rounded-lg border border-slate-300 px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-400 dark:hover:bg-slate-700">
              启用
            </button>
          {/if}
        </div>
        <p class="mt-1 text-xs text-slate-400 dark:text-slate-500">
          不建议手动管理 TOTP，建议使用 Phanerite 启用 TOTP 以自动保存凭据
        </p>
      </div>

      <!-- TOTP 弹窗 -->
      {#if totp_modal}
        <div
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
          onclick={cancel_totp_setup}>
          <div
            class="w-full max-w-lg rounded-xl bg-white p-6 shadow-xl dark:bg-slate-800"
            onclick={e => e.stopPropagation()}>
            <h3 class="text-lg font-semibold text-slate-900 dark:text-slate-100">启用 TOTP</h3>
            <p class="mt-2 text-sm text-slate-500 dark:text-slate-400">
              使用认证器 App 扫描二维码：
            </p>
            <div class="mt-4 flex flex-col items-center gap-3">
              {#if !totp_qr_loaded}
                <div class="flex h-108 w-108 items-center justify-center">
                  <svg class="h-8 w-8 animate-spin text-indigo-500" viewBox="0 0 24 24" fill="none">
                    <circle
                      cx="12"
                      cy="12"
                      r="10"
                      stroke="currentColor"
                      stroke-width="3"
                      opacity="0.2" />
                    <path
                      d="M12 2a10 10 0 0 1 10 10"
                      stroke="currentColor"
                      stroke-width="3"
                      stroke-linecap="round" />
                  </svg>
                </div>
              {:else}
                {@html qr_svg}
              {/if}
              <button
                onclick={copy_secret}
                class="flex cursor-pointer items-center gap-2 rounded bg-slate-100 px-3 py-1.5 font-mono text-sm text-slate-700 hover:bg-slate-200 dark:bg-slate-700 dark:text-slate-300 dark:hover:bg-slate-600">
                {totp_secret}
                {#if totp_copied}
                  <svg
                    class="h-4 w-4 text-green-500"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2.5"
                    ><path
                      d="M20 6L9 17l-5-5"
                      stroke-linecap="round"
                      stroke-linejoin="round" /></svg>
                {:else}
                  <svg
                    class="h-4 w-4 opacity-50"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    ><rect x="9" y="9" width="13" height="13" rx="2" /><path
                      d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"
                      stroke-linecap="round"
                      stroke-linejoin="round" /></svg>
                {/if}
              </button>
            </div>
            <div class="mt-4 flex items-center justify-center gap-2">
              <span class="text-sm text-slate-500 dark:text-slate-400">输入 6 位验证码：</span>
              <input
                type="text"
                bind:value={totp_code}
                maxlength="6"
                placeholder="000000"
                class="w-24 rounded-lg border border-slate-300 px-3 py-2 text-center text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100"
                class:animate-shake={totp_shake} />
            </div>
            <div class="mt-1 h-5 text-center text-sm text-slate-500 dark:text-slate-400">
              {totp_error}
            </div>
            <div class="mt-6 flex justify-end gap-2">
              <button
                onclick={cancel_totp_setup}
                disabled={totp_loading}
                class="cursor-pointer rounded-lg border border-slate-300 px-4 py-2 text-sm text-slate-600 dark:border-slate-600 dark:text-slate-400"
                >取消</button>
              <button
                onclick={submit_totp_setup}
                disabled={totp_loading}
                class="cursor-pointer rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white hover:bg-indigo-700 disabled:opacity-60 dark:bg-indigo-500 dark:hover:bg-indigo-600"
                >{totp_loading ? "…" : "验证"}</button>
            </div>
          </div>
        </div>
      {/if}
    </div>
  {/if}
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
