<!-- The heading of one of the manager's sections, with room for its actions on the right. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = { title: string; description?: string; id?: string; children?: Snippet };
  let { title, description, id, children }: Props = $props();
</script>

<header {id} class="flex scroll-mt-4 items-end justify-between gap-4 pt-2">
  <div>
    <h2 class="text-lg font-semibold tracking-tight">{title}</h2>
    {#if description}<p class="text-sm text-muted-foreground">{description}</p>{/if}
  </div>
  {#if children}<div class="flex items-center gap-2">{@render children()}</div>{/if}
</header>
