<script lang="ts">
  import { SkinViewer } from "@feniota/tiny-skin-viewer";
  import {
    ArrowLeft,
    LoaderCircle,
    PencilLine,
    Upload,
    Trash2,
    RotateCcw,
    Plus,
    Minus,
  } from "@lucide/svelte";
  import { OverlayScrollbarsComponent } from "overlayscrollbars-svelte";
  import { onMount } from "svelte";
  import { pop } from "svelte-spa-router";
  import { fade } from "svelte/transition";

  import Dialog from "@/components/Dialog.svelte";
  import MinecraftCape from "@/components/MinecraftCape.svelte";
  import { toast } from "@/components/toast.svelte";
  import { get_profile, patch_profile, delete_profile, type DetailProfile } from "@/lib/api";
  import { AUTH } from "@/lib/auth.svelte";
  import * as Tooltip from "@/lib/components/ui/tooltip";
  import { t } from "@/lib/i18n.svelte";
  import Trans from "@/lib/Trans.svelte";
  import { transition_tick, FALLBACK_SKIN, cn, type CapeList } from "@/lib/utils";

  const { params = {} }: { params?: { id?: string } } = $props();
  let loading = $state(true);
  let targeted_profile: DetailProfile | null = $state(null);

  let is_grabbing = $state(false);
  let preview_reset_id = $state(0);
  let preview_scale = $state(1.8);
  let webgpu_available: null | boolean = $state(null);

  let change_name_dialog: null | Dialog = $state(null);
  let delete_profile_dialog: null | Dialog = $state(null);
  let upload_skin_dialog: null | Dialog = $state(null);
  let upload_skin_type: boolean = $state(false);
  let upload_slim_arms: boolean = $state(false);

  // ── Rename dialog ──
  let new_name = $state("");
  let rename_loading = $state(false);
  let rename_error = $state("");

  async function handle_rename(e: SubmitEvent) {
    e.preventDefault();
    rename_error = "";
    const name = new_name.trim();
    if (name.length < 3 || name.length > 16) {
      rename_error = t("profile_detail.error_name_length");
      return;
    }
    if (!/^[a-zA-Z0-9_]+$/.test(name)) {
      rename_error = t("profile_detail.error_name_chars");
      return;
    }
    if (name === targeted_profile?.metadata.name) {
      rename_error = t("profile_detail.error_name_same");
      return;
    }
    if (!AUTH.token) return;
    rename_loading = true;
    const resp = await patch_profile(params.id!, name, AUTH.token);
    rename_loading = false;
    if (!resp.success) {
      rename_error = resp.reason;
      return;
    }
    toast(t("toast.profile_rename_success"));
    change_name_dialog?.close();
    // Refresh profile data
    const refreshed = await get_profile(params.id!, true);
    if (refreshed.success) {
      targeted_profile = refreshed.payload;
    }
  }

  // ── Delete dialog ──
  let delete_loading = $state(false);

  async function handle_delete() {
    if (!AUTH.token || !targeted_profile) return;
    delete_loading = true;
    const resp = await delete_profile(targeted_profile.metadata.id, AUTH.token);
    delete_loading = false;
    if (!resp.success) {
      toast(t("toast.profile_delete_fail", { reason: resp.reason }));
      return;
    }
    toast(t("toast.profile_deleted"));
    pop();
  }

  // ── Upload dialog ──
  let selected_file: File | null = $state(null);
  let selected_cape_index: number | null = $state(null);
  let file_preview_url: string | null = $state(null);
  let upload_loading = $state(false);
  let upload_error = $state("");

  function handle_file_select(file: File) {
    upload_error = "";
    if (!file.type.startsWith("image/png")) {
      upload_error = t("profile_detail.error_png_only");
      return;
    }
    if (file.size > 8 * 1024 * 1024) {
      upload_error = t("profile_detail.error_file_size");
      return;
    }
    selected_file = file;
    selected_cape_index = null;
    // Create preview URL
    if (file_preview_url) URL.revokeObjectURL(file_preview_url);
    file_preview_url = URL.createObjectURL(file);
  }

  function handle_cape_select(index: number) {
    selected_cape_index = index;
    selected_file = null;
    if (file_preview_url) {
      URL.revokeObjectURL(file_preview_url);
      file_preview_url = null;
    }
    upload_error = "";
  }

  async function handle_upload() {
    if (!AUTH.token || !targeted_profile) return;
    upload_error = "";
    upload_loading = true;

    const profile_id = targeted_profile.metadata.id;
    const texture_type = upload_skin_type ? "cape" : "skin";

    try {
      let body: FormData;

      if (upload_skin_type && selected_cape_index !== null && capes) {
        // Preset cape: download the image first, then upload
        const cape = capes.capes[selected_cape_index];
        const resp = await fetch(cape.url);
        const blob = await resp.blob();
        body = new FormData();
        body.append("file", blob, "cape.png");
      } else if (selected_file) {
        body = new FormData();
        body.append("file", selected_file);
        if (!upload_skin_type) {
          body.append("model", upload_slim_arms ? "slim" : "");
        }
      } else {
        upload_error = upload_skin_type
          ? t("profile_detail.error_cape_no_selection")
          : t("profile_detail.error_no_selection");
        upload_loading = false;
        return;
      }

      const res = await fetch(`/api/yggdrasil/api/user/profile/${profile_id}/${texture_type}`, {
        method: "PUT",
        headers: { Authorization: `Bearer ${AUTH.token}` },
        body,
      });

      if (!res.ok) {
        const err_text = await res.text().catch(() => "Unknown error");
        upload_error = t("toast.upload_fail_status", { status: res.status, error: err_text });
        upload_loading = false;
        return;
      }

      toast(upload_skin_type ? t("toast.cape_uploaded") : t("toast.skin_uploaded"));
      upload_skin_dialog?.close();

      // Refresh profile data
      const refreshed = await get_profile(params.id!, true);
      if (refreshed.success) {
        targeted_profile = refreshed.payload;
      }
    } catch (e) {
      upload_error = t("toast.upload_fail_generic", {
        error: e instanceof Error ? e.message : String(e),
      });
    }
    upload_loading = false;
  }

  function handle_drop(e: DragEvent) {
    e.preventDefault();
    (e.currentTarget as HTMLElement | null)?.classList.remove("dragover");
    const file = e.dataTransfer?.files[0];
    if (file) handle_file_select(file);
  }

  function reset_upload_state() {
    selected_file = null;
    selected_cape_index = null;
    if (file_preview_url) {
      URL.revokeObjectURL(file_preview_url);
      file_preview_url = null;
    }
    upload_error = "";
    upload_loading = false;
  }

  let capes: null | CapeList = null;

  const fetch_capes: () => Promise<CapeList> = async () => {
    if (capes === null) {
      capes = await fetch("https://assets.ferris.love/phenocryst/capes/capes.json")
        .then(e => e.json())
        .catch(async e => {
          toast(t("toast.cape_list_fail"));
          console.error(e);
          throw e;
        });
    }
    return capes!;
  };

  onMount(() => {
    if (!params.id) {
      // should not happen
      return;
    }
    get_profile(params.id, true).then(resp => {
      if (!resp.success) {
        toast(t("toast.profile_fetch_fail", { reason: resp.reason }));

        if (resp.status === 404) {
          setTimeout(() => {
            pop();
          }, 1500);
        }
        return;
      }
      transition_tick(() => {
        loading = false;
        targeted_profile = resp.payload;
      });
    });
    if (window.navigator.gpu) {
      window.navigator.gpu.requestAdapter().then(ada => {
        webgpu_available = ada !== null;
      });
    } else {
      webgpu_available = false;
    }
  });
</script>

<div class="w-full">
  {#if loading || targeted_profile === null}
    <div
      class="text-primary flex h-[calc(100dvh-var(--spacing)*25)] w-full items-center justify-center">
      <LoaderCircle class="size-16 animate-spin" />
    </div>
  {:else}
    <div class="flex flex-col gap-4">
      <a
        role="link"
        onclick={pop}
        class="text-muted-foreground hover:text-primary aph-tr flex cursor-pointer flex-row items-center hover:underline">
        <ArrowLeft class="mr-2 size-5" />
        {t("profile_detail.back")}
      </a>
      <div class="title">
        {t("profile_detail.title")}
        <span class="font-mojangles">{targeted_profile?.metadata.name}</span>
      </div>

      <div
        class="text-primary-foreground flex flex-row items-center justify-start gap-1 border-y p-4">
        <span>{t("profile_detail.actions")}</span>
        <div class="flex-1"></div>
        <Tooltip.Root>
          <Tooltip.Trigger
            type="button"
            onclick={change_name_dialog?.open}
            class="hover:bg-surface rounded p-0.5">
            <PencilLine class="size-4" />
          </Tooltip.Trigger>
          <Tooltip.Content>{t("profile_detail.rename")}</Tooltip.Content>
        </Tooltip.Root>
        <Tooltip.Root>
          <Tooltip.Trigger
            type="button"
            onclick={delete_profile_dialog?.open}
            class="hover:bg-surface rounded p-0.5">
            <Trash2 class="size-5" />
          </Tooltip.Trigger>
          <Tooltip.Content>{t("profile_detail.delete_profile")}</Tooltip.Content>
        </Tooltip.Root>
      </div>
      <div class="bg-surface/50 relative flex flex-col border-y p-4">
        <div class="mb-4">{t("profile_detail.skin_info")}</div>
        <Tooltip.Root>
          <Tooltip.Trigger
            type="button"
            onclick={() => {
              upload_slim_arms = false;
              upload_skin_type = false;
              upload_skin_dialog?.open();
            }}
            class="hover:text-primary hover:bg-surface text-primary-foreground absolute top-4 right-4 rounded p-0.5">
            <Upload class="size-5" />
          </Tooltip.Trigger>
          <Tooltip.Content>{t("profile_detail.upload_skin")}</Tooltip.Content>
        </Tooltip.Root>
        <div class="flex flex-col items-stretch gap-4 sm:flex-row">
          <div class="group relative h-65 w-45 border">
            {#if webgpu_available === null}
              <div class="flex h-65 w-45 flex-col items-center justify-center">
                <LoaderCircle class="text-foreground size-10 animate-spin" />
              </div>
            {:else if webgpu_available === false}
              <div class="flex h-65 w-45 flex-col items-center justify-center p-3 text-center">
                <p><Trans k="profile_detail.webgpu_unavailable" /></p>
                <a
                  class="text-primary cursor-pointer underline"
                  href="https://phenocryst.ferris.love/zh/aphanite/troubleshooting#webgpu-not-available"
                  >{t("profile_detail.view_details")}</a>
              </div>
            {:else}
              <!-- 3D preview -->
              <div
                onpointerdown={() => (is_grabbing = true)}
                onpointerup={() => (is_grabbing = false)}
                onpointerleave={() => (is_grabbing = false)}
                onpointercancel={() => (is_grabbing = false)}>
                <SkinViewer
                  class={cn("cursor-grab [&.active]:cursor-grabbing", is_grabbing && "active")}
                  resetId={preview_reset_id}
                  scale={preview_scale}
                  width={180}
                  height={260}
                  capeUrl={targeted_profile.skin?.cape}
                  isSlim={targeted_profile.skin?.model === "slim"}
                  skinUrl={targeted_profile.skin?.skin ?? FALLBACK_SKIN} />
                <Tooltip.Root>
                  <Tooltip.Trigger
                    type="button"
                    onclick={() => {
                      preview_reset_id += 1;
                      preview_scale = 1.8;
                    }}
                    tabindex={-1}
                    class="bg-background hover:bg-surface hover:text-primary absolute right-2 bottom-2 rounded-lg border p-2 opacity-0 transition-[opacity,background-color,text-color] duration-200 group-hover:opacity-100">
                    <RotateCcw />
                  </Tooltip.Trigger>
                  <Tooltip.Content side="left">{t("profile_detail.reset_skin")}</Tooltip.Content>
                </Tooltip.Root>
                <div
                  onclick={() => (preview_reset_id += 1)}
                  class="bg-background *:hover:bg-surface *:hover:text-primary absolute bottom-2 left-2 flex flex-col items-center rounded-lg border opacity-0 transition-opacity *:p-1 group-hover:opacity-100">
                  <button
                    tabindex="-1"
                    type="button"
                    onclick={() => (preview_scale += 0.2)}
                    class="rounded-t-lg border-b"><Plus /></button>
                  <button
                    tabindex="-1"
                    type="button"
                    onclick={() => (preview_scale -= 0.2)}
                    class="rounded-b-lg"><Minus /></button>
                </div>
              </div>
            {/if}
          </div>
          <div class="flex flex-col">
            <div class="flex flex-row">
              <div class="">
                <img
                  class="pixelated w-50 border p-4"
                  src={targeted_profile.skin?.skin ?? FALLBACK_SKIN} />
              </div>
              <MinecraftCape cape_url={targeted_profile.skin?.cape} class="ml-4 w-32 border" />
            </div>
            <div class="mt-4 flex flex-1 flex-row items-end text-sm sm:mt-0">
              <div>
                {t("profile_detail.skin_model")}<span
                  class="text-glaucous-700 dark:text-glaucous-400 bg-foreground/10 rounded-full border px-1.5 pt-0.5 font-mono"
                  >{(targeted_profile.skin?.model ?? "default").toUpperCase()}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
      <div>
        <div>
          UUID：<code class="bg-surface rounded px-1 py-0.5">{targeted_profile.metadata.id}</code>
        </div>
      </div>
    </div>
  {/if}
</div>

<Dialog bind:this={change_name_dialog}>
  <div class="text-primary-foreground text-lg">{t("profile_detail.rename_dialog_title")}</div>
  <div><Trans k="profile_detail.rename_dialog_body" /></div>
  <div>{t("profile_detail.original_name", { name: targeted_profile?.metadata.name })}</div>
  <form class="flex flex-col" onsubmit={handle_rename}>
    <label for="new_profile_name">{t("profile_detail.new_name_label")}</label>
    <input
      id="new_profile_name"
      class="font-mojangles placeholder:text-muted-foreground input-surface mt-1 block w-full rounded-lg border px-3 py-2 text-sm transition"
      placeholder="New_Name"
      type="text"
      bind:value={new_name} />
    <div class="text-muted-foreground mt-4">
      <ul class="list-outside list-disc pl-4">
        <li><Trans k="profile_detail.name_rule_length" /></li>
        <li><Trans k="profile_detail.name_rule_chars" /></li>
        <li><Trans k="profile_detail.name_rule_offensive" /></li>
      </ul>
    </div>
    {#if rename_error}
      <div class="mt-2 text-sm text-red-500">{rename_error}</div>
    {/if}
    <button
      type="submit"
      disabled={rename_loading}
      class="bg-primary loading:bg-muted mt-4 flex flex-row items-center justify-center gap-2 rounded-lg px-3 py-2 text-white disabled:opacity-50 md:justify-start md:self-end">
      {#if rename_loading}
        <LoaderCircle class="size-5 animate-spin" />
      {:else}
        <span>{t("common.submit")}</span>
      {/if}
    </button>
  </form>
</Dialog>

<Dialog bind:this={delete_profile_dialog}>
  <div class="text-primary-foreground text-lg">{t("profile_detail.delete_dialog_title")}</div>
  <div>
    <Trans k="profile_detail.delete_dialog_body" opts={{ name: targeted_profile?.metadata.name }} />
  </div>
  <button
    type="button"
    disabled={delete_loading}
    onclick={handle_delete}
    class="bg-primary loading:bg-muted mt-4 flex flex-row items-center justify-center gap-2 rounded-lg px-3 py-2 text-white disabled:opacity-50 md:justify-start md:self-end">
    {#if delete_loading}
      <LoaderCircle class="size-5 animate-spin" />
    {:else}
      <span>{t("profile_detail.confirm_delete")}</span>
    {/if}
  </button>
</Dialog>

<Dialog bind:this={upload_skin_dialog} onclose={reset_upload_state} class="upload-skin-dialog">
  <div class="text-primary-foreground upload-skin-title text-lg">
    {t("profile_detail.upload_title")}
  </div>
  <div class="upload-skin-select flex flex-row self-stretch rounded border *:focus:ring-0">
    <button
      class={cn("aph-tr flex-1 rounded-l py-0.5", !upload_skin_type && "bg-surface")}
      onclick={() =>
        transition_tick(() => {
          upload_skin_type = false;
          reset_upload_state();
        })}>{t("profile_detail.skin_tab")}</button>
    <div class="border-l"></div>
    <button
      class={cn("aph-tr flex-1 rounded-r py-0.5", upload_skin_type && "bg-surface")}
      onclick={() =>
        transition_tick(() => {
          upload_skin_type = true;
          reset_upload_state();
        })}>{t("profile_detail.cape_tab")}</button>
  </div>

  {#if upload_skin_type}
    <!-- Cape preset selection -->
    <div class="mt-4">{t("profile_detail.select_cape")}</div>

    <div class="relative inline-grid h-56 place-items-center *:col-start-1 *:row-start-1">
      {#await fetch_capes()}
        <div transition:fade class="flex h-56 items-center justify-center">
          <LoaderCircle class="size-8 animate-spin" />
        </div>
      {:then capes_list}
        <OverlayScrollbarsComponent
          class="aph h-56 w-full px-2 ring-0"
          options={{ overflow: { x: "scroll", y: "hidden" }, scrollbars: { autoHide: "leave" } }}>
          <div class="flex max-w-full min-w-0 flex-row gap-4 py-2">
            {#each capes_list.capes as cape, i}
              <button
                transition:fade
                type="button"
                onclick={() => handle_cape_select(i)}
                class={cn(
                  "hover:bg-surface flex h-52 flex-col items-center rounded-lg border p-4",
                  "transition-[padding,background-color,border-width,border-color] last:mr-4",
                  selected_cape_index === i &&
                    "bg-surface border-primary border-3 p-[calc(var(--spacing)*4-2px)]",
                )}>
                <MinecraftCape class="mt-2 h-28 w-17.5" cape_url={cape.url} />
                <span class="mt-4 text-center">{cape.name_zh}</span>
              </button>
            {/each}
            <div class="w-1">&nbsp;</div>
          </div>
        </OverlayScrollbarsComponent>
      {/await}
    </div>
    <div class="mt-4">{t("common.or")}</div>
  {:else}
    <div class="flex flex-row justify-between pt-4">
      <span>{t("profile_detail.arm_width")}</span>
      <div class="flex flex-row rounded border transition-colors duration-200 *:px-4 *:py-0.5">
        <button
          type="button"
          onclick={() => (upload_slim_arms = false)}
          class={cn("rounded-l border-r", !upload_slim_arms && "bg-surface")}
          >{t("profile_detail.wide_arms")}</button>
        <button
          type="button"
          onclick={() => (upload_slim_arms = true)}
          class={cn("rounded-r", upload_slim_arms && "bg-surface")}
          >{t("profile_detail.slim_arms")}</button>
      </div>
    </div>
  {/if}

  <!-- File upload area -->
  <div
    role="button"
    tabindex="0"
    onclick={e => {
      e.stopPropagation();
      document.getElementById("upload-file-input")?.click();
    }}
    ondragover={e => {
      e.preventDefault();
      (e.currentTarget as HTMLElement).classList.add("dragover");
    }}
    ondragleave={e => {
      (e.currentTarget as HTMLElement).classList.remove("dragover");
    }}
    ondrop={handle_drop}
    id="upload-file-box"
    class={cn(
      "dragover:border-3 dragover:border-primary dragover:bg-primary/30 mt-4 flex",
      "h-60 items-center justify-center rounded-lg border p-4",
      "transition-[background-color,border-color,border-width] duration-200",
      upload_skin_type && !selected_file && "mt-0 h-30",
    )}>
    {#if file_preview_url}
      <div class="flex flex-col items-center gap-2">
        <img src={file_preview_url} class="max-h-40 rounded object-contain" />
        <div class="text-muted-foreground text-sm">{selected_file?.name}</div>
        <button
          type="button"
          class="text-primary hover:text-primary-foreground text-sm underline"
          onclick={e => {
            e.stopPropagation();
            document.getElementById("upload-file-input")?.click();
          }}>
          {t("profile_detail.change_file")}
        </button>
      </div>
    {:else}
      <div class="text-center" id="upload-box-tip-text">
        <Upload class="text-muted-foreground mb-2 inline-block size-8" />
        <div>{t("profile_detail.drag_prompt")}</div>
        <div class="text-muted-foreground text-sm">
          <Trans k="profile_detail.file_requirements" />
        </div>
      </div>
    {/if}
  </div>
  {#if upload_error}
    <div class="mt-2 text-sm text-red-500">{upload_error}</div>
  {/if}

  <button
    id="upload-skin-confirm-btn"
    disabled={upload_loading || (!selected_file && selected_cape_index === null)}
    onclick={handle_upload}
    class="bg-primary loading:bg-muted mt-4 flex flex-row items-center justify-center gap-2 rounded-lg px-3 py-2 text-white disabled:opacity-50 md:justify-start md:self-end">
    {#if upload_loading}
      <LoaderCircle class="size-5 animate-spin" />
      <span class="loading:text-muted-surface-foreground">{t("profile_detail.uploading")}</span>
    {:else}
      <span>{t("profile_detail.upload")}</span>
    {/if}
  </button>
</Dialog>

<input
  id="upload-file-input"
  type="file"
  accept=".png,image/png"
  class="hidden"
  onchange={e => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (file) handle_file_select(file);
  }} />

<style>
  .pixelated {
    image-rendering: pixelated;
  }
  .upload-skin-dialog {
    view-transition-name: upload-skin-dialog;
  }
  .upload-skin-title {
    view-transition-name: upload-skin-title;
  }
  .upload-skin-select {
    view-transition-name: upload-skin-select;
  }
  #upload-skin-confirm-btn {
    view-transition-name: upload-skin-confirm-btn;
  }
  #upload-file-box {
    view-transition-name: upload-file-box;
  }
  #upload-box-tip-text {
    view-transition-name: upload-box-tip-text;
  }
</style>
