<!-- A status next to a name, such as "active": a dot for the meaning, text at a readable size,
     and a hairline pill that holds the two together. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { DOT_LIVE } from "$lib/tones";

  let { children }: { children: Snippet } = $props();
</script>

<span
  class="inline-flex shrink-0 items-center gap-1.5 rounded-full border border-border px-2 py-px text-xs font-normal text-muted-foreground"
>
  <span class="size-1.5 rounded-full {DOT_LIVE}"></span>
  {@render children()}
</span>
