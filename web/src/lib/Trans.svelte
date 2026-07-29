<!-- @component
Trans component handles typographic spacing between text segments in i18n translations,
specifically for CJK-Latin mixed text.

It uses the i18n marker character "|" as segment delimiters in translation strings.
The component checks for browser support of `text-autospace: normal` CSS property.

When supported, the marker characters are stripped and the raw translated string
is rendered directly (relying on CSS `text-autospace` for spacing).

When not supported, the translated string is split on markers and each segment is
rendered with explicit spacing between them:
  - By default (space=true), a <Space/> component is inserted between segments.
    <Space/> is a 0.125ic wide inline-block <div>, whose width exactly matches
    text-autospace:normal behavior.
  - When space=false, a regular 0.25em wide inline-block span is used instead.

Props:
  - k: i18n key string
  - opts: interpolation options for the i18n translation (default: {})
  - space: whether to use <Space/> between segments (default: true).
    Set to false for a regular space-width gap.
-->
<script lang="ts">
  import Space from "@/components/Space.svelte";
  import { t } from "@/lib/i18n.svelte";

  const MARKER = "|";

  const {
    k,
    opts = {},
    space = true,
  }: {
    /** i18n key */
    k: string;
    /** i18n interpolation options */
    opts?: Record<string, unknown>;
    /** Whether to use <Space/> between segments (default: true). Set to false for regular space. */
    space?: boolean;
  } = $props();

  const supported = window.CSS.supports("text-autospace", "normal");

  // on-demand string manipulations for performance
  const raw = $derived(space && supported ? t(k, opts).replaceAll(MARKER, "") : "");
  const segments = $derived(space && supported ? [] : t(k, opts).split(MARKER));
</script>

{#if space && supported}{raw}{:else}{#each segments as segment, i}{segment}{#if i < segments.length - 1}{#if space}<Space />{:else}<span
          class="inline-block w-[0.25em]"></span
        >{/if}{/if}{/each}{/if}
