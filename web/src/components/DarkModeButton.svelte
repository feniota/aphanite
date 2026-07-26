<script lang="ts">
  import { Moon, Sun, LaptopMinimal } from "@lucide/svelte";

  import * as Tooltip from "@/lib/components/ui/tooltip";
  import { set_dark_mode, type DarkMode } from "@/lib/darkmode";

  const cycle: DarkMode[] = ["light", "dark", "system"];
  let mode = $state((localStorage.getItem("aphanite.dark-mode") as DarkMode) ?? "system");

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

<Tooltip.Root>
  <Tooltip.Trigger
    type="button"
    class="text-muted-foreground hover:bg-surface cursor-pointer rounded-sm p-1 transition-colors duration-200 focus:ring"
    onclick={toggle}>
    <Icon class="size-5"></Icon>
  </Tooltip.Trigger>
  <Tooltip.Content>{labels[mode]}</Tooltip.Content>
</Tooltip.Root>
