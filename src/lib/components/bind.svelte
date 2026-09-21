<!-- A keybind drawn as keys: Shift + Q, with alternatives separated by a slash. The keys are
     borderless chips, chosen over outlined, raised and solid ones for being the quietest that
     still read as keys. -->
<script lang="ts">
  import { Kbd } from "$lib/components/ui/kbd";
  import { parseBind } from "$lib/binds";

  let { value }: { value: string | null } = $props();

  const chords = $derived(parseBind(value));
  const KEY = "h-[1.375rem] min-w-[1.375rem] rounded-sm px-1.5 text-xs";
</script>

{#if chords.length === 0}
  <!-- No key: an empty chip, the size of a real one so that columns of keys stay aligned. -->
  <span class="inline-flex shrink-0 bg-foreground/8 {KEY}" role="img" aria-label="Not bound" title="Not bound"></span>
{:else}
  <span class="inline-flex items-center gap-1">
    {#each chords as chord, index (index)}
      {#if index > 0}<span class="text-faint">/</span>{/if}
      <span class="inline-flex items-center gap-0.5">
        {#each chord as key, position (position)}
          <Kbd class="bg-foreground/12 text-foreground {KEY}">{key}</Kbd>
        {/each}
      </span>
    {/each}
  </span>
{/if}
