<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { watchTheme } from "$lib/theme";

  let { children } = $props();

  onMount(() => {
    // The page is up: the mark that `app.html` shows while it loads has done its job.
    document.getElementById("splash")?.remove();
    return watchTheme();
  });
</script>

{@render children()}
