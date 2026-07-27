<script lang="ts">
  import "@/lib/darkmode";
  import "../../node_modules/overlayscrollbars/styles/overlayscrollbars.css";
  import { OverlayScrollbarsComponent } from "overlayscrollbars-svelte";
  import { onMount } from "svelte";

  import Router from "@/components/HomePageRouter.svelte";
  import SideBar from "@/components/HomePageSideBar.svelte";
  import { toast } from "@/components/toast.svelte";
  import Toast from "@/components/Toast.svelte";
  import TopBar from "@/components/TopBar.svelte";
  import { AUTH } from "@/lib/auth.svelte";
  import { Provider as TooltipProvider } from "@/lib/components/ui/tooltip";
  import { t } from "@/lib/i18n.svelte";

  onMount(() => {
    AUTH.validate().then(v => {
      if (!v) {
        toast(t("toast.session_expired"));
        setTimeout(() => {
          window.location.replace(
            `${window.location.origin}${window.location.pathname}/login?redirected_from_dashboard=true`,
          );
        });
      }
    });
  });
</script>

<OverlayScrollbarsComponent
  class="aph h-dvh w-full"
  options={{ scrollbars: { autoHide: "leave" }, overflow: { x: "hidden" } }}>
  <TooltipProvider delayDuration={500}>
    <TopBar></TopBar>
    <div class="min-h-dvh pt-15">
      <div
        class="aph-container mx-auto flex flex-row sm:border-r sm:border-l lg:mx-0 lg:w-screen lg:max-w-none! lg:border-none">
        <SideBar class="flex-3"></SideBar>
        <div
          class="border-border/50 min-h-[calc(100dvh-var(--spacing)*15)] min-w-0 flex-9 p-5 2xl:mx-[7vw] 2xl:border-x">
          <Router />
        </div>
      </div>
    </div>
    <Toast></Toast>
  </TooltipProvider>
</OverlayScrollbarsComponent>

<style>
  :global(body) {
    overflow: hidden;
  }
</style>
