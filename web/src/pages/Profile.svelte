<script lang="ts">
  import QRCode from "qrcode";

  import {
    getMe,
    updateMe,
    changePassword,
    issueTotp,
    activateTotp,
    deleteTotp,
    createVerification,
    completeVerification,
    ApiError,
  } from "../lib/api";
  import type { User } from "../lib/api";
  import { auth } from "../lib/auth.svelte";
  import { show } from "../lib/toast.svelte";

  let user = $state<User | null>(null);
  let loading = $state(true);

  // 编辑个人信息
  let editName = $state("");
  let editEmail = $state("");
  let editing = $state(false);
  let saveLoading = $state(false);

  // 修改密码弹窗
  let pwdModal = $state(false);
  let pwdStep = $state(1);
  let pwdMethod = $state<"password" | "totp">("password");
  let pwdOldPassword = $state("");
  let pwdOtpCode = $state("");
  let pwdNewPassword = $state("");
  let pwdConfirm = $state("");
  let pwdLoading = $state(false);
  let pwdError = $state("");

  // TOTP
  let totpEnabled = $state(false);
  let totpModal = $state(false);
  let totpSecret = $state("");
  let totpUrl = $state("");
  let totpCode = $state("");
  let totpLoading = $state(false);
  let totpError = $state("");
  let totpQrLoaded = $state(false);
  let totpCopied = $state(false);
  let qrSvg = $state("");
  let disableLoading = $state(false);
  let totpShake = $state(false);

  $effect(() => {
    if (totpUrl) {
      totpQrLoaded = false;
      QRCode.toString(totpUrl, {
        type: "svg",
        color: { dark: "#000", light: "#fff" },
        width: 432,
      }).then((svg) => {
        qrSvg = svg;
        totpQrLoaded = true;
      });
    }
  });

  function copySecret() {
    navigator.clipboard.writeText(totpSecret);
    totpCopied = true;
    setTimeout(() => (totpCopied = false), 2000);
  }

  $effect(() => {
    if (!auth.token) return;
    getMe(auth.token)
      .then((u) => {
        user = u;
        auth.setSession(auth.token!, u);
        // 检查 TOTP 是否已启用
        return createVerification(u.email, "totp").then(() => (totpEnabled = true));
      })
      .catch(() => {})
      .finally(() => (loading = false));
  });

  function startEdit() {
    if (!user) return;
    editName = user.name;
    editEmail = user.email;
    editing = true;
  }

  function cancelEdit() {
    editing = false;
  }

  async function saveProfile(e: SubmitEvent) {
    e.preventDefault();
    if (!auth.token) return;
    saveLoading = true;
    try {
      const updated = await updateMe(auth.token, {
        name: editName || undefined,
        email: editEmail,
      });
      user = updated;
      auth.setSession(auth.token!, updated);
      editing = false;
      show("保存成功");
    } catch (err) {
      show(err instanceof ApiError ? err.message : "保存失败");
    } finally {
      saveLoading = false;
    }
  }

  function openPwdModal() {
    pwdStep = 1;
    pwdMethod = totpEnabled ? "totp" : "password";
    pwdOldPassword = "";
    pwdOtpCode = "";
    pwdNewPassword = "";
    pwdConfirm = "";
    pwdError = "";
    pwdModal = true;
  }

  function closePwdModal() {
    pwdModal = false;
  }

  function nextPwdStep() {
    pwdError = "";
    pwdStep = 2;
  }

  async function submitPasswordChange() {
    if (!auth.token) return;
    if (pwdMethod === "password" && !pwdOldPassword) {
      pwdError = "请输入旧密码";
      return;
    }
    if (pwdMethod === "totp" && pwdOtpCode.length !== 6) {
      pwdError = "请输入 6 位 TOTP 验证码";
      return;
    }
    if (pwdNewPassword !== pwdConfirm) {
      pwdError = "两次输入的密码不一致";
      return;
    }
    pwdError = "";
    pwdLoading = true;
    try {
      let otp_token: string | undefined;
      if (pwdMethod === "totp") {
        const { id } = await createVerification(user!.email, "totp");
        const res = await completeVerification(id, pwdOtpCode);
        otp_token = res.otp_token;
      }
      await changePassword(auth.token, {
        old_password: pwdMethod === "password" ? pwdOldPassword : undefined,
        otp_token,
        new_password: pwdNewPassword,
      });
      show("密码修改成功");
      closePwdModal();
    } catch (err) {
      show(err instanceof ApiError ? err.message : "修改失败");
    } finally {
      pwdLoading = false;
    }
  }

  // ── TOTP ──

  async function openTotpModal() {
    if (!auth.token) return;
    totpError = "";
    totpCopied = false;
    totpQrLoaded = false;
    totpCode = "";
    totpLoading = true;
    totpModal = true;
    try {
      const res = await issueTotp(auth.token);
      totpSecret = res.secret;
      totpUrl = res.otpauth_url;
    } catch (err) {
      totpError = err instanceof ApiError ? err.message : "请求失败";
    } finally {
      totpLoading = false;
    }
  }

  async function cancelTotpSetup() {
    totpModal = false;
    // 取消时删除刚刚签发的 TOTP 密钥
    if (auth.token) {
      try {
        await deleteTotp(auth.token);
      } catch {
        /* ignore */
      }
    }
  }

  async function submitTotpSetup() {
    if (!auth.token || !user) return;
    totpError = "";
    if (totpCode.length !== 6) {
      totpShake = true;
      setTimeout(() => (totpShake = false), 500);
      return;
    }
    totpLoading = true;
    try {
      const { id } = await createVerification(user.email, "totp");
      const { otp_token } = await completeVerification(id, totpCode);
      await activateTotp(auth.token, otp_token);
      totpEnabled = true;
      totpModal = false;
    } catch {
      totpError = "验证码错误，请重试";
      totpShake = true;
      setTimeout(() => (totpShake = false), 500);
    } finally {
      totpLoading = false;
    }
  }

  async function disableTotp() {
    if (!auth.token) return;
    disableLoading = true;
    try {
      await deleteTotp(auth.token);
      totpEnabled = false;
    } catch {
      /* ignore */
    } finally {
      disableLoading = false;
    }
  }
</script>

<div class="mx-auto max-w-2xl p-8">
  <h1 class="text-2xl font-bold text-slate-900 dark:text-slate-100">个人资料</h1>

  {#if loading}
    <p class="mt-4 text-sm text-slate-400">加载中…</p>
  {:else if user}
    <!-- 基本信息 -->
    <div
      class="mt-6 rounded-xl border border-slate-200 bg-white p-6 dark:border-slate-700 dark:bg-slate-800">
      <h2 class="text-lg font-semibold text-slate-900 dark:text-slate-100">基本信息</h2>

      {#if editing}
        <form class="mt-4 space-y-4" onsubmit={saveProfile}>
          <div>
            <label class="block text-sm font-medium text-slate-700 dark:text-slate-300"
              >用户名</label>
            <input
              type="text"
              bind:value={editName}
              class="mt-1 block w-full max-w-xs rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100" />
          </div>
          <div>
            <label class="block text-sm font-medium text-slate-700 dark:text-slate-300">邮箱</label>
            <input
              type="email"
              bind:value={editEmail}
              required
              class="mt-1 block w-full max-w-xs rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100" />
          </div>
          <div class="flex gap-2">
            <button
              type="submit"
              disabled={saveLoading}
              class="cursor-pointer rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white hover:bg-indigo-700 disabled:opacity-60 dark:bg-indigo-500 dark:hover:bg-indigo-600">
              {saveLoading ? "保存中…" : "保存"}
            </button>
            <button
              type="button"
              onclick={cancelEdit}
              disabled={saveLoading}
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
          onclick={startEdit}
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
            onclick={openPwdModal}
            class="cursor-pointer rounded-lg border border-slate-300 px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-400 dark:hover:bg-slate-700">
            修改密码
          </button>
        </div>
      </div>

      <!-- 修改密码弹窗 -->
      {#if pwdModal}
        <div
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
          onclick={closePwdModal}>
          <div
            class="w-full max-w-md rounded-xl bg-white p-6 shadow-xl dark:bg-slate-800"
            onclick={(e) => e.stopPropagation()}>
            <h3 class="text-lg font-semibold text-slate-900 dark:text-slate-100">修改密码</h3>

            {#if pwdStep === 1}
              <p class="mt-4 text-sm text-slate-500 dark:text-slate-400">选择验证方式</p>
              <div class="mt-3 space-y-3">
                <button
                  onclick={() => {
                    pwdMethod = "password";
                    nextPwdStep();
                  }}
                  class="w-full cursor-pointer rounded-xl border-2 border-slate-200 p-4 text-left transition hover:border-slate-300 dark:border-slate-600 dark:hover:border-slate-500">
                  <span class="text-sm font-medium text-slate-900 dark:text-slate-100">旧密码</span>
                  <span class="mt-1 block text-xs text-slate-500 dark:text-slate-400"
                    >使用当前密码验证身份</span>
                </button>
                <button
                  onclick={() => {
                    pwdMethod = "totp";
                    nextPwdStep();
                  }}
                  disabled={!totpEnabled}
                  class="w-full cursor-pointer rounded-xl border-2 border-slate-200 p-4 text-left transition hover:border-slate-300 disabled:opacity-40 dark:border-slate-600 dark:hover:border-slate-500">
                  <span class="text-sm font-medium text-slate-900 dark:text-slate-100">TOTP</span>
                  <span class="mt-1 block text-xs text-slate-500 dark:text-slate-400"
                    >使用认证器应用生成的验证码</span>
                </button>
              </div>
              <div class="mt-6 flex justify-end">
                <button
                  onclick={closePwdModal}
                  class="cursor-pointer rounded-lg border border-slate-300 px-4 py-2 text-sm text-slate-600 dark:border-slate-600 dark:text-slate-400"
                  >取消</button>
              </div>
            {:else if pwdStep === 2}
              <p class="mt-4 text-sm text-slate-500 dark:text-slate-400">
                验证：{pwdMethod === "password" ? "旧密码" : "TOTP"}
              </p>
              {#if pwdMethod === "password"}
                <input
                  type="password"
                  bind:value={pwdOldPassword}
                  placeholder="当前密码"
                  class="mt-2 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100" />
              {:else}
                <input
                  type="text"
                  bind:value={pwdOtpCode}
                  maxlength="6"
                  placeholder="6 位 TOTP 验证码"
                  class="mt-2 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100" />
              {/if}
              <p class="mt-4 text-sm text-slate-500 dark:text-slate-400">设置新密码</p>
              <input
                type="password"
                bind:value={pwdNewPassword}
                placeholder="新密码"
                class="mt-2 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100" />
              <input
                type="password"
                bind:value={pwdConfirm}
                placeholder="确认新密码"
                class="mt-2 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100" />
              {#if pwdError}
                <p class="mt-2 text-sm text-red-600 dark:text-red-400">{pwdError}</p>
              {/if}
              <div class="mt-6 flex justify-between">
                <button
                  onclick={() => (pwdStep = 1)}
                  disabled={pwdLoading}
                  class="cursor-pointer rounded-lg border border-slate-300 px-4 py-2 text-sm text-slate-600 dark:border-slate-600 dark:text-slate-400"
                  >← 上一步</button>
                <button
                  onclick={submitPasswordChange}
                  disabled={pwdLoading}
                  class="cursor-pointer rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white hover:bg-indigo-700 disabled:opacity-60 dark:bg-indigo-500 dark:hover:bg-indigo-600"
                  >{pwdLoading ? "…" : "确认修改"}</button>
              </div>
            {/if}
          </div>
        </div>
      {/if}
      <div class="pt-6">
        <div class="flex items-center justify-between">
          <h3 class="font-medium text-slate-700 dark:text-slate-300">TOTP</h3>
          {#if totpEnabled}
            <button
              onclick={disableTotp}
              disabled={disableLoading}
              class="cursor-pointer rounded-lg border border-red-300 px-3 py-1.5 text-sm text-red-600 hover:bg-red-50 disabled:opacity-60 dark:border-red-700 dark:text-red-400 dark:hover:bg-red-950">
              {disableLoading ? "…" : "禁用"}
            </button>
          {:else}
            <button
              onclick={openTotpModal}
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
      {#if totpModal}
        <div
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
          onclick={cancelTotpSetup}>
          <div
            class="w-full max-w-lg rounded-xl bg-white p-6 shadow-xl dark:bg-slate-800"
            onclick={(e) => e.stopPropagation()}>
            <h3 class="text-lg font-semibold text-slate-900 dark:text-slate-100">启用 TOTP</h3>
            <p class="mt-2 text-sm text-slate-500 dark:text-slate-400">
              使用认证器 App 扫描二维码：
            </p>
            <div class="mt-4 flex flex-col items-center gap-3">
              {#if !totpQrLoaded}
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
                {@html qrSvg}
              {/if}
              <button
                onclick={copySecret}
                class="flex cursor-pointer items-center gap-2 rounded bg-slate-100 px-3 py-1.5 font-mono text-sm text-slate-700 hover:bg-slate-200 dark:bg-slate-700 dark:text-slate-300 dark:hover:bg-slate-600">
                {totpSecret}
                {#if totpCopied}
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
                bind:value={totpCode}
                maxlength="6"
                placeholder="000000"
                class="w-24 rounded-lg border border-slate-300 px-3 py-2 text-center text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-100"
                class:animate-shake={totpShake} />
            </div>
            <div class="mt-1 h-5 text-center text-sm text-slate-500 dark:text-slate-400">
              {totpError}
            </div>
            <div class="mt-6 flex justify-end gap-2">
              <button
                onclick={cancelTotpSetup}
                disabled={totpLoading}
                class="cursor-pointer rounded-lg border border-slate-300 px-4 py-2 text-sm text-slate-600 dark:border-slate-600 dark:text-slate-400"
                >取消</button>
              <button
                onclick={submitTotpSetup}
                disabled={totpLoading}
                class="cursor-pointer rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white hover:bg-indigo-700 disabled:opacity-60 dark:bg-indigo-500 dark:hover:bg-indigo-600"
                >{totpLoading ? "…" : "验证"}</button>
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
