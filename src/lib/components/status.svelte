<!-- A status next to a name, such as "active": a dot for the meaning, a word so that it does
     not rest on colour alone, in a hairline pill at a readable size. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { DOT_LIVE, DOT_WAITING } from "$lib/tones";

  type Props = {
    /** What the dot says: that this is live right now, or that something awaits the user. */
    tone?: "live" | "waiting";
    title?: string;
    children: Snippet;
  };
  let { tone = "live", title, children }: Props = $props();
</script>

<span
  class="inline-flex h-4 shrink-0 items-center gap-1 rounded-full border border-border px-1.5 text-[0.6875rem] leading-none font-normal text-muted-foreground"
  {title}
>
  <span class="size-1.5 rounded-full {tone === "live" ? DOT_LIVE : DOT_WAITING}"></span>
  {@render children()}
</span>
