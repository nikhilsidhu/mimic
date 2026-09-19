<!--
  Replaces the Windows title bar on frameless windows. Follows Microsoft's title bar
  guidance so it still feels native: 32px tall, 46px full-bleed caption buttons drawn
  with the system's own icon font, the whole bar draggable, double-click to maximize,
  and everything dimmed while the window is inactive.
-->
<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import type { Snippet } from "svelte";

  let { title = "mimic", children }: { title?: string; children?: Snippet } = $props();

  const appWindow = getCurrentWindow();
  let maximized = $state(false);
  let focused = $state(true);

  onMount(() => {
    appWindow.isMaximized().then((value) => (maximized = value));
    const stops = [
      appWindow.onResized(async () => (maximized = await appWindow.isMaximized())),
      // Windows also reports a blur when a click moves focus into the web view, so a
      // blur only counts if the window is still inactive a moment later.
      appWindow.onFocusChanged(({ payload }) => {
        if (payload) focused = true;
        else setTimeout(async () => (focused = await appWindow.isFocused()), 100);
      }),
    ];
    return () => stops.forEach((stop) => stop.then((unlisten) => unlisten()));
  });
</script>

<!-- `data-tauri-drag-region` only applies to the element that carries it, so the title
     and spacer carry it too; buttons must not. -->
<!-- `app-region` (inline, as CSS checkers do not know it) adds touch and pen dragging on Windows. -->
<header class="titlebar" class:inactive={!focused} data-tauri-drag-region style="app-region: drag">
  <span class="title" data-tauri-drag-region>{title}</span>
  <div class="extra" data-tauri-drag-region>
    {@render children?.()}
  </div>
  <div class="caption" style="app-region: no-drag">
    <button onclick={() => appWindow.minimize()} aria-label="Minimize" tabindex="-1">&#xE921;</button>
    <button onclick={() => appWindow.toggleMaximize()} aria-label={maximized ? "Restore" : "Maximize"} tabindex="-1">
      {#if maximized}&#xE923;{:else}&#xE922;{/if}
    </button>
    <button class="close" onclick={() => appWindow.close()} aria-label="Close" tabindex="-1">&#xE8BB;</button>
  </div>
</header>

<style>
  .titlebar {
    display: flex;
    align-items: stretch;
    flex: none;
    height: 32px;
    user-select: none;
  }

  .title {
    display: flex;
    align-items: center;
    padding-left: 16px;
    font-size: 12px;
    color: var(--muted-foreground);
  }

  .extra {
    display: flex;
    flex: 1;
    align-items: center;
    min-width: 0;
    padding: 0 16px;
  }

  .caption {
    display: flex;
  }

  .caption button {
    width: 46px;
    height: 32px;
    /* The system's caption glyphs: Windows 11 first, Windows 10 as the fallback. */
    font-family: "Segoe Fluent Icons", "Segoe MDL2 Assets";
    font-size: 10px;
    color: var(--foreground);
    transition: background-color 0.1s;
  }

  .caption button:hover {
    background: color-mix(in oklch, var(--foreground) 9%, transparent);
  }

  .caption button:active {
    background: color-mix(in oklch, var(--foreground) 6%, transparent);
  }

  /* Windows' own close-button red. */
  .caption .close:hover {
    background: #c42b1c;
    color: #fff;
  }

  .caption .close:active {
    background: #c42b1ce6;
    color: #fff;
  }

  .inactive .title,
  .inactive .caption button:not(:hover) {
    opacity: 0.5;
  }
</style>
