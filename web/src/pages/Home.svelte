<script lang="ts">
  import "../../../node_modules/overlayscrollbars/styles/overlayscrollbars.css";
  import { Copy, Eye, LoaderCircle, Plus } from "@lucide/svelte";
  import { OverlayScrollbarsComponent } from "overlayscrollbars-svelte";
  import { onMount } from "svelte";
  import { link, push } from "svelte-spa-router";
  import { fade } from "svelte/transition";

  import MinecraftAvatar from "@/components/MinecraftAvatar.svelte";
  import { toast } from "@/components/toast.svelte";
  import { AUTH } from "@/lib/auth.svelte";
  import * as Tooltip from "@/lib/components/ui/tooltip";
  import { t } from "@/lib/i18n.svelte";

  let profiles_loading = $state(true);
  let profiles = $derived(AUTH.profiles.value);

  onMount(() => {
    AUTH.init_profiles().then(r => {
      if (!r) {
        toast(t("toast.profiles_fetch_fail"));
      }
      profiles_loading = false;
    });
  });

  function copy_uuid(uuid: string) {
    return async () => {
      await navigator.clipboard.writeText(uuid);
      toast(t("toast.uuid_copied"));
    };
  }
</script>

<div class="flex w-full flex-col gap-4">
  <div class="title">
    {t("home.welcome_back")}<span class="text-primary-foreground font-semibold"
      >{AUTH.user?.name}</span
    >{t("home.and")}
  </div>

  <div class="flex w-full flex-col border-y p-4">
    <div class="flex flex-row items-center justify-between">
      <div class="text-muted-foreground">
        <span class="text-primary-foreground">{t("home.your_profiles")}</span><span class="mx-2"
          >·</span
        ><a
          use:link
          href="/profiles"
          class="hover:text-primary-foreground cursor-pointer transition-colors hover:underline"
          >{t("home.view_more")}</a>
      </div>
      <Tooltip.Root>
        <Tooltip.Trigger
          type="button"
          onclick={() => push("/profiles?action=create")}
          class="hover:bg-surface hover:text-primary-foreground rounded p-1">
          <Plus class="size-5" />
        </Tooltip.Trigger>
        <Tooltip.Content>{t("home.create_new_profile")}</Tooltip.Content>
      </Tooltip.Root>
    </div>
    <div class="mt-4 mb-2">
      {#if typeof profiles?.length === "number" && profiles?.length > 0}
        <OverlayScrollbarsComponent
          class="aph ring-0"
          options={{ overflow: { x: "scroll", y: "hidden" }, scrollbars: { autoHide: "leave" } }}>
          <div class="flex max-w-full min-w-0 flex-row gap-4">
            {#each profiles as profile}
              <div transition:fade class="card shrink-0 text-center">
                <MinecraftAvatar class="mt-2 mb-6 inline-block" skin_url={profile.skin?.skin} />
                <div class="font-mojangles w-full text-center">{profile.metadata.name}</div>
                <div
                  class="text-primary-foreground mt-2 flex flex-row items-stretch justify-center">
                  <Tooltip.Root>
                    <Tooltip.Trigger>
                      {#snippet child({ props })}
                        <a
                          use:link
                          href={`/profile/${profile.metadata.id}`}
                          {...props}
                          class="hover:bg-surface rounded p-0.5"
                          type="button">
                          <Eye class="size-5" />
                        </a>
                      {/snippet}
                    </Tooltip.Trigger>
                    <Tooltip.Content>{t("home.view_details")}</Tooltip.Content>
                  </Tooltip.Root>
                  <div class="mx-2"></div>
                  <Tooltip.Root>
                    <Tooltip.Trigger
                      class="hover:bg-surface rounded p-0.5"
                      type="button"
                      onclick={copy_uuid(profile.metadata.id)}>
                      <Copy class="my-0.5 size-4" />
                    </Tooltip.Trigger>
                    <Tooltip.Content>{t("home.copy_uuid")}</Tooltip.Content>
                  </Tooltip.Root>
                </div>
              </div>
            {/each}
          </div>
        </OverlayScrollbarsComponent>
      {/if}
      {#if profiles?.length === 0}
        <div class="text-muted-foreground flex-1 self-stretch">
          {t("home.no_profiles")}<button
            type="button"
            onclick={() => push("/profiles?action=create")}
            class="text-primary-foreground hover:text-primary underline"
            >{t("home.create_now")}</button
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
