<!-- A small fact next to a name, in a hairline pill at a readable size: a status such as
     "active", with a dot for what is live, or a count with an icon of its own. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { DOT_LIVE } from "$lib/tones";

  type Props = {
    /** The dot that says this is live right now. Left out for a pill that brings an icon. */
    dot?: boolean;
    title?: string;
    children: Snippet;
  };
  let { dot = true, title, children }: Props = $props();
</script>

<span
  class="inline-flex h-4 shrink-0 items-center gap-1 rounded-full border border-border px-1.5 text-[0.6875rem] leading-none font-normal text-muted-foreground"
  {title}
>
  {#if dot}<span class="size-1.5 rounded-full {DOT_LIVE}"></span>{/if}
  {@render children()}
</span>
