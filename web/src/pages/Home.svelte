<script lang="ts">
  import "../../../node_modules/overlayscrollbars/styles/overlayscrollbars.css";
  import { Copy, Eye, LoaderCircle, Plus } from "@lucide/svelte";
  import { OverlayScrollbarsComponent } from "overlayscrollbars-svelte";
  import { onMount } from "svelte";
  import { link } from "svelte-spa-router";
  import { fade } from "svelte/transition";

  import MinecraftAvatar from "@/components/MinecraftAvatar.svelte";
  import { show } from "@/components/toast.svelte";
  import { AUTH } from "@/lib/auth.svelte";

  let profiles_loading = $state(true);
  let profiles = $derived(AUTH.profiles.value);

  onMount(() => {
    AUTH.init_profiles().then(r => {
      if (!r) {
        show(`获取玩家档案列表失败`);
      }
      profiles_loading = false;
    });
  });

  function copy_uuid(uuid: string) {
    return async () => {
      await navigator.clipboard.writeText(uuid);
      show("档案 UUID 已复制到剪贴板。");
    };
  }
</script>

<div class="flex w-full flex-col gap-4">
  <div class="mt-4 mb-4 text-3xl">
    <span class="">欢迎回来，</span><span class="text-primary-foreground font-semibold"
      >{AUTH.user?.name}</span
    ><span class="">。</span>
  </div>

  <div class="flex w-full flex-col border-y p-4">
    <div class="flex flex-row items-center justify-between">
      <div class="text-muted-foreground">
        <span class="text-primary-foreground">你的玩家档案</span><span class="mx-2">·</span><a
          use:link
          href="/profiles"
          class="hover:text-primary-foreground cursor-pointer transition-colors hover:underline"
          >查看更多</a>
      </div>
      <button
        title="创建新档案"
        type="button"
        class="hover:bg-surface hover:text-primary-foreground rounded p-1">
        <Plus class="size-5" />
      </button>
    </div>
    <div class="mt-4 mb-2">
      {#if typeof profiles?.length === "number" && profiles?.length > 0}
        <OverlayScrollbarsComponent
          class="aph ring-0"
          options={{ overflow: { x: "scroll", y: "hidden" }, scrollbars: { autoHide: "leave" } }}>
          <div class="flex max-w-full min-w-0 flex-row gap-4">
            {#each profiles as profile}
              <div transition:fade class="card hover:bg-surface aph-tr shrink-0 text-center">
                <MinecraftAvatar class="mt-2 mb-6 inline-block" skin_url={profile.skin?.skin} />
                <div class="w-full text-center">{profile.metadata.name}</div>
                <div class="text-muted-foreground mt-2 flex flex-row items-stretch justify-center">
                  <button class="hover:bg-muted rounded p-0.5" type="button" title="查看详情"
                    ><Eye class="size-5" /></button>
                  <div class="mx-2"></div>
                  <button
                    class="hover:bg-muted rounded p-0.5"
                    type="button"
                    title="复制 UUID"
                    onclick={copy_uuid(profile.metadata.id)}><Copy class="my-0.5 size-4" /></button>
                </div>
              </div>
            {/each}
          </div>
        </OverlayScrollbarsComponent>
      {/if}
      {#if profiles?.length === 0}
        <div class="text-muted-foreground flex-1 self-stretch">
          看起来你还没有玩家档案。<button
            type="button"
            class="text-primary-foreground hover:text-primary underline">现在创建</button
          >？
        </div>
      {/if}
      {#if profiles_loading}
        <div class="flex-1 self-stretch text-center">
          <LoaderCircle class="mx-auto size-10 animate-spin" />
        </div>
      {/if}
    </div>
  </div>
</div>
