<script lang="ts">
  import { Moon, Sun, LaptopMinimal } from "@lucide/svelte";

  import * as Tooltip from "@/lib/components/ui/tooltip";
  import { set_dark_mode, type DarkMode } from "@/lib/darkmode";
  import { t } from "@/lib/i18n.svelte";

  const cycle: DarkMode[] = ["light", "dark", "system"];
  let mode = $state((localStorage.getItem("aphanite.dark-mode") as DarkMode) ?? "system");

  const icons = {
    light: Sun,
    dark: Moon,
    system: LaptopMinimal,
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
  <Tooltip.Content
    >{mode === "light"
      ? t("common.light_mode")
      : mode === "dark"
        ? t("common.dark_mode")
        : t("common.system_mode")}</Tooltip.Content>
</Tooltip.Root>
