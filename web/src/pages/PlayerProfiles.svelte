<script lang="ts">
  import { onMount } from "svelte";

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
</script>

<div class="w-full"></div>
