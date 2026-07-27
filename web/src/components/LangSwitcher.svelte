<script lang="ts">
  import { Languages } from "@lucide/svelte";

  import * as DropdownMenu from "@/lib/components/ui/dropdown-menu";
  import { change_language, get_current_language } from "@/lib/i18n.svelte";
  import { cn } from "@/lib/utils";

  let lang = $state(get_current_language());

  function set_lang(l: string) {
    change_language(l);
    lang = l;
  }

  const ITEMS = [
    { label: "简体中文", value: "zh-CN" },
    { label: "English", value: "en" },
  ] as const;

  let {
    class: className,
    tooltip_side = "top",
  }: {
    class?: string;
    /** @default "top" */
    tooltip_side?: "top" | "bottom" | "left" | "right";
  } = $props();
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger
    type="button"
    class={cn(
      "text-muted-foreground hover:bg-surface cursor-pointer rounded-sm p-1",
      "transition-colors duration-200 focus:ring",
      className,
    )}>
    <Languages class="size-5" />
  </DropdownMenu.Trigger>
  <DropdownMenu.Content>
    <DropdownMenu.Group>
      {#each ITEMS as item}
        <DropdownMenu.Item onclick={() => set_lang(item.value)}>
          <div class="flex w-full items-center justify-between gap-2">
            <span>{item.label}</span>
            {#if lang === item.value}
              <span class="bg-primary text-primary-foreground inline-flex size-1.5 rounded-full"
              ></span>
            {/if}
          </div>
        </DropdownMenu.Item>
      {/each}
    </DropdownMenu.Group>
  </DropdownMenu.Content>
</DropdownMenu.Root>
