<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  const VISIBLE_MS = 4000;

  let message = $state("");
  let timer: ReturnType<typeof setTimeout> | undefined;

  function show(text: string) {
    message = text;
    clearTimeout(timer);
    timer = setTimeout(() => getCurrentWindow().hide(), VISIBLE_MS);
  }

  onMount(() => {
    invoke<string>("current_notice").then(show);
    const unlisten = listen<string>("notice", (event) => show(event.payload));
    return () => {
      clearTimeout(timer);
      unlisten.then((stop) => stop());
    };
  });
</script>

<main>{message}</main>

<style>
  main {
    box-sizing: border-box;
    height: 100vh;
    padding: 0 16px;
    display: flex;
    align-items: center;
    border: 1px solid var(--border);
    background: var(--popover);
    color: var(--popover-foreground);
    font-size: 12px;
    line-height: 1.4;
  }
</style>
