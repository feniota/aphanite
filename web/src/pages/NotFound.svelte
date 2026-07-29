<script lang="ts">
  import { ChevronRight, Bot } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";

  import { t } from "@/lib/i18n.svelte";
  import Trans from "@/lib/Trans.svelte";
  import "@/lib/darkmode";
  import { cn } from "@/lib/utils";

  let phase = $state(0);

  // Compute the base path for navigation links
  const home_url = window.__APANITE_BASE__ ? `${window.__APANITE_BASE__}/` : "/";

  onMount(async () => {
    function sleep(duration: number): Promise<void> {
      return new Promise(resolve => {
        setTimeout(resolve, duration);
      });
    }

    await sleep(300);
    phase = 1;
    await sleep(700);
    phase = 2;
    await sleep(2000);
    phase = 3;
    await sleep(700);
    phase = 4;
    await sleep(1500);
    phase = 5;
  });
</script>

<div class={cn("min-w-vw flex min-h-dvh flex-col items-center justify-center")}>
  <div class="relative flex h-[50dvh] w-[85vw] flex-col border pt-10 md:w-[60vw]">
    {#if phase > 0}
      <div
        in:fly={{ x: -20, duration: 400 }}
        class="text-primary-foreground flex flex-row items-center px-4">
        <ChevronRight class="text-primary mr-4" />
        <span><Trans k="not-found.user-input" /></span>
      </div>
    {/if}

    {#if phase > 1}
      <div class="mx-4 my-4 border-b"></div>
      <div transition:fade={{ duration: 200 }} class="flex flex-row items-center px-4 italic">
        <Bot class="text-primary mr-4" />
        <span class={cn(phase === 2 && "animate-pulse")}
          ><Trans k={phase === 2 ? "not-found.thinking" : "not-found.thought"} /></span>
      </div>
    {/if}

    {#if phase > 2}
      <div transition:fly={{ y: 10, duration: 400 }} class="my-4 pl-14">
        <Trans k="not-found.sure-thing" />
      </div>
    {/if}

    {#if phase > 3}
      <div
        transition:fly={{ y: 20, duration: 500, delay: 100 }}
        class="mx-6 mb-6 flex-1 overflow-hidden rounded-2xl border bg-white dark:bg-black">
        {#if phase > 4}
          <div
            transition:fade
            class={cn(
              "relative flex h-full w-full flex-col items-center justify-center",
              "overflow-hidden bg-linear-to-br from-[#0f2027] via-[#203a43] to-[#2c5364] p-8",
            )}>
            <!-- 背景装饰光晕 -->
            <div
              class={cn(
                "pointer-events-none absolute -top-20 -left-20 h-64 w-64 rounded-full",
                "bg-[#f7971e] opacity-50 blur-[80px]",
              )}>
            </div>
            <div
              class={cn(
                "pointer-events-none absolute -right-20 -bottom-20 h-64 w-64 rounded-full",
                "bg-[#2c5364] opacity-50 blur-[80px]",
              )}>
            </div>

            <!-- 浮动动画容器 -->
            <div class={cn("relative z-10 animate-[float_6s_ease-in-out_infinite] text-center")}>
              <h1
                class={cn(
                  "bg-linear-to-r from-[#f7971e] to-[#ffd200] bg-clip-text text-[120px]",
                  "leading-none font-black text-transparent",
                  "drop-shadow-[0_0_30px_rgba(255,210,0,0.3)] select-none",
                )}>
                404
              </h1>
              <p class={cn("mt-4 text-2xl text-white opacity-90")}>
                <Trans k="not-found.404-title" />
              </p>
              <a
                href={home_url}
                class={cn(
                  "mt-8 inline-block rounded-full border border-white/30 bg-white/10 px-8 py-3",
                  "text-white backdrop-blur-md transition-all hover:bg-white/20",
                  "hover:shadow-[0_0_15px_rgba(255,255,255,0.2)]",
                )}>
                <Trans k="not-found.back" />
              </a>
            </div>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<!-- 必须加上这个全局样式，Tailwind 默认不支持自定义的 float 动画 -->
<style>
  @keyframes float {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-20px);
    }
  }
</style>
