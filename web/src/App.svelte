<script lang="ts">
  import "@/lib/darkmode";
  import { onMount } from "svelte";

  import Router from "@/components/HomePageRouter.svelte";
  import SideBar from "@/components/HomePageSideBar.svelte";
  import { show } from "@/components/toast.svelte";
  import Toast from "@/components/Toast.svelte";
  import TopBar from "@/components/TopBar.svelte";
  import { AUTH } from "@/lib/auth.svelte";

  onMount(() => {
    AUTH.validate().then(v => {
      if (!v) {
        show("登录状态失效，请重新登录。");
        setTimeout(() => {
          window.location.replace(
            `${window.location.origin}${window.location.pathname}/login?redirected_from_dashboard=true`,
          );
        });
      }
    });
  });
</script>

<TopBar></TopBar>
<div class="min-h-dvh pt-15">
  <div
    class="aph-container mx-auto flex flex-row sm:border-r sm:border-l lg:mx-0 lg:w-screen lg:max-w-none! lg:border-none">
    <SideBar class="flex-3"></SideBar>
    <div
      class="border-border/50 min-h-[calc(100dvh-var(--spacing)*15)] flex-9 min-w-0 p-5 2xl:mx-[7vw] 2xl:border-x">
      <Router />
    </div>
  </div>
</div>
<Toast></Toast>
