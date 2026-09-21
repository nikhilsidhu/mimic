<!-- A keybind drawn as keycaps: Shift + Q, with alternatives separated by a slash. -->
<script lang="ts">
  import { Kbd } from "$lib/components/ui/kbd";
  import { parseBind } from "$lib/binds";

  let { value }: { value: string | null } = $props();

  const chords = $derived(parseBind(value));
</script>

{#if chords.length === 0}
  <!-- No key: an empty keycap, the size of a real one so that columns of keys stay aligned. -->
  <span
    class="inline-flex h-[1.375rem] min-w-[1.375rem] shrink-0 rounded-sm border border-dashed border-foreground/25"
    role="img"
    aria-label="Not bound"
    title="Not bound"
  ></span>
{:else}
  <span class="inline-flex items-center gap-1">
    {#each chords as chord, index (index)}
      {#if index > 0}<span class="text-faint">/</span>{/if}
      <span class="inline-flex items-center gap-0.5">
        {#each chord as key, position (position)}
          <Kbd class="h-[1.375rem] min-w-[1.375rem] rounded-sm border border-foreground/25 bg-foreground/10 px-1.5 text-xs text-foreground">{key}</Kbd>
        {/each}
      </span>
    {/each}
  </span>
{/if}
