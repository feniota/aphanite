<script lang="ts">
  import { cn } from "@/lib/utils";
  import { FALLBACK_SKIN } from "@/lib/utils";

  let canvas_elem: HTMLCanvasElement | null = $state(null);
  const { skin_url, class: className }: { skin_url?: string; class?: string } = $props();
  const skin = $derived(skin_url ?? FALLBACK_SKIN);

  // Pixel buffer at 2× native resolution (native face area is 8×8)
  const W = 16;
  const H = 16;

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
      ctx.drawImage(skin_img, 8, 8, 8, 8, 0, 0, W, H);
      ctx.drawImage(skin_img, 40, 8, 8, 8, 0, 0, W, H);
    };
  });
</script>

<div class={cn("relative isolate inline-flex size-12 items-center justify-center", className)}>
  <canvas width={W} height={H} bind:this={canvas_elem} class="pixelated block h-full w-full">
  </canvas>
</div>

<style>
  .pixelated {
    image-rendering: pixelated;
  }
</style>
