<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { watchTheme } from "$lib/theme";

  let { children } = $props();

  onMount(watchTheme);
</script>

{@render children()}
