<script lang="ts">
  import "@/lib/darkmode";
  import Router from "@/components/HomePageRouter.svelte";
  import SideBar from "@/components/HomePageSideBar.svelte";
  import TopBar from "@/components/TopBar.svelte";
</script>

<TopBar></TopBar>
<div class="min-h-dvh pt-15">
  <div
    class="aph-container mx-auto flex flex-row border-r border-l lg:mx-0 lg:w-screen lg:max-w-none! lg:border-none">
    <SideBar class="flex-3"></SideBar>
    <div class="flex-9 p-5"><Router /></div>
  </div>
</div>
