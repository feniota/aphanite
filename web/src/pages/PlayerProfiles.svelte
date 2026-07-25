<script lang="ts">
  import { LoaderCircle, Plus, ListChecks, X, Check, Copy, Trash2 } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { replace, router } from "svelte-spa-router";
  import { fade } from "svelte/transition";

  import Dialog from "@/components/Dialog.svelte";
  import MinecraftAvatar from "@/components/MinecraftAvatar.svelte";
  import Space from "@/components/Space.svelte";
  import { toast } from "@/components/toast.svelte";
  import { create_profile, delete_profile } from "@/lib/api";
  import { AUTH } from "@/lib/auth.svelte";
  import { cn, transition_tick } from "@/lib/utils";

  let selection_mode = $state(false);
  let profiles_loading = $state(true);
  let profiles = $derived(AUTH.profiles.value);
  let selected_profiles: { [x: string]: boolean } = $state({});

  // ── Create dialog ──
  let create_dialog: null | Dialog = $state(null);
  let new_profile_name = $state("");
  let create_loading = $state(false);
  let create_error = $state("");

  async function handle_create(e: SubmitEvent) {
    e.preventDefault();
    create_error = "";
    const name = new_profile_name.trim();
    if (name.length < 3 || name.length > 16) {
      create_error = "名称长度需在 3–16 个字符之间";
      return;
    }
    if (!/^[a-zA-Z0-9_]+$/.test(name)) {
      create_error = "名称只能包含英文字母、数字和下划线";
      return;
    }
    if (!AUTH.token) return;
    create_loading = true;
    const resp = await create_profile(name, AUTH.token);
    create_loading = false;
    if (!resp.success) {
      create_error = resp.reason;
      return;
    }
    toast(`档案「${name}」已创建`);
    create_dialog?.close();
    await AUTH.refresh_profiles();
  }

  // ── Batch delete ──
  let batch_delete_loading = $state(false);
  let batch_delete_dialog: null | Dialog = $state(null);
  let batch_delete_targets: string[] = $state([]);

  function confirm_batch_delete() {
    const ids = Object.entries(selected_profiles)
      .filter(([, selected]) => selected)
      .map(([id]) => id);
    if (ids.length === 0) return;
    batch_delete_targets = ids;
    batch_delete_dialog?.open();
  }

  async function execute_batch_delete() {
    if (!AUTH.token) return;
    batch_delete_loading = true;
    let success_count = 0;
    let fail_count = 0;
    for (const id of batch_delete_targets) {
      const resp = await delete_profile(id, AUTH.token);
      if (resp.success) success_count++;
      else fail_count++;
    }
    batch_delete_loading = false;
    batch_delete_dialog?.close();
    if (fail_count > 0) {
      toast(`已删除 ${success_count} 个档案，${fail_count} 个删除失败`);
    } else {
      toast(`已删除 ${success_count} 个档案`);
    }
    selection_mode = false;
    selected_profiles = {};
    await AUTH.refresh_profiles();
  }

  function copy(uuid: string, copied_anim: (arg0: boolean) => void) {
    return async () => {
      try {
        await window.navigator.clipboard.writeText(uuid);
        toast("已复制到剪贴板");
        copied_anim(true);
        setTimeout(() => copied_anim(false), 1000);
      } catch (e) {
        toast("复制失败，请重试");
        console.error(e);
      }
    };
  }

  onMount(() => {
    AUTH.init_profiles().then(r => {
      if (!r) {
        toast(`获取玩家档案列表失败`);
        return;
      }
      transition_tick(() => {
        profiles_loading = false;
      });
    });
  });

  // Auto-open create dialog if navigated from Home "+" button
  $effect(() => {
    if (create_dialog) {
      const params = new URLSearchParams(router.querystring);
      if (params.get("action") === "create") {
        create_dialog.open();
        // Clean up the query param from the URL
        replace("/profiles");
      }
    }
  });
</script>

<div class="w-full">
  {#if profiles_loading}
    <div
      class="text-primary flex h-[calc(100dvh-var(--spacing)*25)] w-full items-center justify-center">
      <LoaderCircle class="size-16 animate-spin" />
    </div>
  {:else}
    <div class="flex flex-col gap-4">
      <div class="title">玩家档案列表</div>
      <div
        class={cn(
          "text-primary-foreground flex flex-row items-center justify-start gap-1 border-y p-4 transition-colors duration-200",
          selection_mode &&
            "border-primary bg-surface border px-[calc(var(--spacing)*4-1px)] *:hover:bg-white/20 ",
        )}>
        <span class="relative inline-grid place-items-center *:col-start-1 *:row-start-1">
          {#if !selection_mode}
            <span transition:fade>管理操作</span>
          {:else}
            <span transition:fade>批量管理</span>
          {/if}
        </span>
        {#if selection_mode}
          <button
            type="button"
            onclick={() => {
              selection_mode = false;
              selected_profiles = {};
            }}
            class="hover:bg-surface rounded"
            title="退出选择模式"
            transition:fade>
            <X class="size-5" />
          </button>
        {/if}
        <div class="flex-1"></div>
        {#if selection_mode}
          <button
            transition:fade
            type="button"
            title="删除"
            disabled={batch_delete_loading}
            onclick={confirm_batch_delete}
            class="hover:bg-surface rounded disabled:opacity-50"><Trash2 class="size-5" /></button>
        {/if}
        <button
          type="button"
          title="创建"
          onclick={create_dialog?.open}
          class="hover:bg-surface rounded"><Plus class="size-5" /></button>
        <button
          type="button"
          title="多选"
          class="hover:bg-surface rounded"
          onclick={() => {
            selection_mode = !selection_mode;
            if (!selection_mode) selected_profiles = {};
          }}><ListChecks class="size-5" /></button>
      </div>
      <div class="grid grid-cols-1 gap-4 lg:grid-cols-2 xl:grid-cols-3">
        {#each profiles as profile}
          {const self_selected = $derived(
            selection_mode && selected_profiles[profile.metadata.id] === true,
          )}
          <!-- No use:link: it would bind a click event on the <a>, causing EVERY click (even on the copy button below) to navigate -->
          <a
            transition:fade
            onclick={e => {
              if (selection_mode) {
                e.preventDefault();
                console.log("click!");
                selected_profiles[profile.metadata.id] = !selected_profiles[profile.metadata.id];
              }
            }}
            href={`#/profile/${profile.metadata.id}`}
            class={cn(
              "card hover:bg-surface/50 aph-tr flex cursor-pointer flex-col items-center outline-0 -outline-offset-2",
              "relative outline-transparent transition-[outline-width,outline-color] duration-200 md:flex-row",
              selection_mode && "focus:ring-0",
              self_selected && "outline-primary bg-surface hover:bg-surface outline-2",
            )}>
            {#if self_selected}
              <div transition:fade={{ duration: 200 }} class="text-primary absolute top-2 right-2">
                <Check />
              </div>
            {/if}
            <div class="self-center text-center md:mr-4 md:flex-1">
              <MinecraftAvatar class="my-2 inline-block" skin_url={profile.skin?.skin} />
            </div>
            <div class="hidden self-stretch border-l md:block"></div>
            <div class="text-muted-foreground md:flex-3 md:pl-4">
              <div class="w-full text-center md:text-start">
                <span class="hidden md:inline">游戏内名称：</span><span
                  class="text-primary-foreground font-mojangles">{profile.metadata.name}</span>
              </div>
              <div class="w-full text-center md:text-start">
                {let copied_anim = $state(false)}
                <span class="hidden md:inline">UUID：</span><span
                  class="text-primary-foreground hover:bg-muted aph-tr cursor-text rounded px-1 py-0.5 font-mono not-dark:bg-white/50 not-dark:hover:bg-white/90"
                  >{profile.metadata.id}<button
                    class="aph-tr hover:text-primary-foreground hover:bg-surface ml-1 inline size-4 cursor-pointer rounded p-0.5 focus:ring-0"
                    type="button"
                    onclick={e => {
                      e.preventDefault();
                      copy(profile.metadata.id, b => (copied_anim = b))();
                    }}
                    >{#if !copied_anim}<Copy class="size-3" />{:else}<Check
                        class="size-3" />{/if}</button
                  ></span>
              </div>
            </div>
          </a>
        {/each}
      </div>
    </div>
  {/if}
</div>

<Dialog bind:this={batch_delete_dialog}>
  <div class="text-primary-foreground text-lg">删除档案</div>
  <div>
    确定要删除<Space /><span class="font-semibold">{batch_delete_targets.length}</span
    ><Space />个档案吗？该操作不可恢复。
  </div>
  <button
    type="button"
    disabled={batch_delete_loading}
    onclick={execute_batch_delete}
    class="bg-primary loading:bg-muted mt-4 flex flex-row items-center justify-center gap-2 rounded-lg px-3 py-2 text-white disabled:opacity-50 md:justify-start md:self-end">
    {#if batch_delete_loading}
      <LoaderCircle class="size-5 animate-spin" />
    {:else}
      <span>确认删除</span>
    {/if}
  </button>
</Dialog>

<Dialog bind:this={create_dialog}>
  <div class="text-primary-foreground text-lg">创建玩家档案</div>
  <div>为新角色输入一个名称。</div>
  <form class="flex flex-col" onsubmit={handle_create}>
    <label for="new_profile_name">档案名称</label>
    <input
      id="new_profile_name"
      class="font-mojangles placeholder:text-muted-foreground input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition"
      placeholder="New_Name"
      type="text"
      bind:value={new_profile_name} />
    <div class="text-muted-foreground mt-4">
      <ul class="list-outside list-disc pl-4">
        <li>长度不小于<Space />3<Space />个字符，且不大于<Space />16<Space />个字符。</li>
        <li>仅包含英文字母、数字和下划线。</li>
      </ul>
    </div>
    {#if create_error}
      <div class="mt-2 text-sm text-red-500">{create_error}</div>
    {/if}
    <button
      type="submit"
      disabled={create_loading}
      class="bg-primary loading:bg-muted mt-4 flex flex-row items-center justify-center gap-2 rounded-lg px-3 py-2 text-white disabled:opacity-50 md:justify-start md:self-end">
      {#if create_loading}
        <LoaderCircle class="size-5 animate-spin" />
      {:else}
        <span>创建</span>
      {/if}
    </button>
  </form>
</Dialog>
