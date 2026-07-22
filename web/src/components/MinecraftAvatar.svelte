<script lang="ts">
  import { cn } from "@/lib/utils";
  const FALLBACK_SKIN = "https://assets.ferris.love/phenocryst/steve.png";

  let canvas_elem: HTMLCanvasElement | null = $state(null);
  const {
    skin_url,
    class: className,
    scale,
  }: { skin_url?: string; class?: string; scale?: number } = $props();
  const skin = $derived(skin_url ?? FALLBACK_SKIN);

  $effect(() => {
    if (!canvas_elem) return;

    const skin_img = new Image();
    skin_img.loading = "eager";
    skin_img.src = skin;

    skin_img.onload = () => {
      if (!canvas_elem) {
        console.warn("MinecraftAvatar.svelte — `canvas_elem` unexpectedly disappeared");
        return;
      }

      const ctx = canvas_elem.getContext("2d")!;
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(skin_img, 8, 8, 8, 8, 0, 0, 8 * (scale ?? 6), 8 * (scale ?? 6));
      ctx.drawImage(skin_img, 40, 8, 8, 8, 0, 0, 8 * (scale ?? 6), 8 * (scale ?? 6));
    };
  });

  $effect(() => {
    if (!canvas_elem) return;
    if (!scale) return;
    const ctx = canvas_elem.getContext("2d")!;
    ctx.scale(scale, scale);
  });
</script>

<canvas
  class={cn("size-12", className)}
  width={8 * (scale ?? 6)}
  height={8 * (scale ?? 6)}
  bind:this={canvas_elem}></canvas>
