<script lang="ts">
  import { Moon, Sun, LaptopMinimal } from "@lucide/svelte";

  import { set_dark_mode, type DarkMode } from "@/lib/darkmode";

  const cycle: DarkMode[] = ["light", "dark", "system"];
  let mode = $state(localStorage.getItem("aphanite.dark-mode") as DarkMode ?? "system");

  const icons = {
    light: Sun,
    dark: Moon,
    system: LaptopMinimal,
  } as const;

  const labels = {
    light: "浅色模式",
    dark: "深色模式",
    system: "跟随系统",
  } as const;

  function toggle() {
    const idx = cycle.indexOf(mode);
    mode = cycle[(idx + 1) % cycle.length];
    set_dark_mode(mode);
  }

  const Icon = $derived(icons[mode]);
</script>

<button
  type="button"
  class="text-muted-foreground hover:bg-surface cursor-pointer rounded-sm p-1 transition-colors duration-200 focus:ring"
  onclick={toggle}
  title={labels[mode]}>
  <Icon class="size-5"></Icon>
</button>

