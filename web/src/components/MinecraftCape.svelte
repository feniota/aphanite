<script lang="ts">
  import { TriangleAlert } from "@lucide/svelte";

  import { cn } from "@/lib/utils";

  let canvas_elem: HTMLCanvasElement | null = $state(null);
  let has_cape = $state(false);

  const { cape_url, class: className }: { cape_url?: string; class?: string } = $props();

  // Pixel buffer at 2× native resolution, sufficient for crisp rendering
  // at any display size since the canvas scales with CSS image-rendering: pixelated
  const W = 20;
  const H = 32;

  $effect(() => {
    has_cape = false;

    if (!cape_url || !canvas_elem) return;

    const img = new Image();
    img.crossOrigin = "anonymous";
    img.loading = "eager";
    img.src = cape_url;

    let cancelled = false;

    img.onload = () => {
      if (cancelled || !canvas_elem) return;
      const ctx = canvas_elem.getContext("2d")!;
      ctx.imageSmoothingEnabled = false;
      ctx.clearRect(0, 0, canvas_elem.width, canvas_elem.height);
      ctx.drawImage(img, 1, 0, 10, 16, 0, 0, W, H);
      has_cape = true;
    };

    img.onerror = () => {
      if (!cancelled) has_cape = false;
    };

    return () => {
      cancelled = true;
    };
  });
</script>

<div class={cn("relative isolate inline-flex items-center justify-center", className)}>
  <canvas width={W} height={H} bind:this={canvas_elem} class="pixelated block h-auto w-full"
  ></canvas>
  {#if !has_cape}
    <div class="text-muted-foreground absolute inset-0 m-auto h-1/2 w-full text-center">无披风</div>
  {/if}
</div>

<style>
  .pixelated {
    image-rendering: pixelated;
  }
</style>
