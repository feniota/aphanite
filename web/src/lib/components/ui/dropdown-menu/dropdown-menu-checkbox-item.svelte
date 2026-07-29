<script lang="ts">
  import CheckIcon from "@lucide/svelte/icons/check";
  import MinusIcon from "@lucide/svelte/icons/minus";
  import { DropdownMenu as DropdownMenuPrimitive } from "bits-ui";
  import type { Snippet } from "svelte";

  import { cn, type WithoutChildrenOrChild } from "@/lib.js";

  let {
    ref = $bindable(null),
    checked = $bindable(false),
    indeterminate = $bindable(false),
    class: className,
    children: childrenProp,
    ...restProps
  }: WithoutChildrenOrChild<DropdownMenuPrimitive.CheckboxItemProps> & {
    children?: Snippet;
  } = $props();
</script>

<DropdownMenuPrimitive.CheckboxItem
  bind:ref
  bind:checked
  bind:indeterminate
  data-slot="dropdown-menu-checkbox-item"
  class={cn(
    "focus:bg-accent focus:text-accent-foreground focus:**:text-accent-foreground",
    "gap-1.5 rounded-md py-1 pr-8 pl-1.5 text-sm data-inset:pl-7",
    "relative flex cursor-default items-center [&_svg:not([class*='size-'])]:size-4",
    "outline-hidden select-none data-[disabled]:pointer-events-none",
    "data-[disabled]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0",
    className,
  )}
  {...restProps}>
  {#snippet children({ checked, indeterminate })}
    <span
      class="pointer-events-none absolute right-2 flex items-center justify-center"
      data-slot="dropdown-menu-checkbox-item-indicator">
      {#if indeterminate}
        <MinusIcon />
      {:else if checked}
        <CheckIcon />
      {/if}
    </span>
    {@render childrenProp?.()}
  {/snippet}
</DropdownMenuPrimitive.CheckboxItem>
