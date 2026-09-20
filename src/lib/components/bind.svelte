<!-- A keybind drawn as keycaps: Shift + Q, with alternatives separated by a slash. -->
<script lang="ts">
  import { Kbd } from "$lib/components/ui/kbd";
  import { parseBind } from "$lib/binds";

  let { value }: { value: string | null } = $props();

  const chords = $derived(parseBind(value));
</script>

{#if chords.length === 0}
  <span class="text-faint">none</span>
{:else}
  <span class="inline-flex items-center gap-1">
    {#each chords as chord, index (index)}
      {#if index > 0}<span class="text-faint">/</span>{/if}
      <span class="inline-flex items-center gap-0.5">
        {#each chord as key, position (position)}
          <Kbd class="border border-border bg-muted px-1.5 text-foreground">{key}</Kbd>
        {/each}
      </span>
    {/each}
  </span>
{/if}
