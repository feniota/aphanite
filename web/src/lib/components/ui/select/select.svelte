<script lang="ts">
  import { Select as SelectPrimitive } from "bits-ui";

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let {
    open = $bindable(false),
    value = $bindable<string>(),
    type = "single",
    ...restProps
  }: Record<string, any> = $props();
</script>

<SelectPrimitive.Root {type} bind:open bind:value {...restProps} />
