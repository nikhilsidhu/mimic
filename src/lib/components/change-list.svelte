<!-- Which settings changed and how: binds as keys, everything else as plain values. -->
<script lang="ts">
  import BellOff from "@lucide/svelte/icons/bell-off";
  import FromTo from "$lib/components/from-to.svelte";
  import { isBind, settingLabel } from "$lib/binds";
  import type { Change } from "$lib/api";

  // With `onmute`, every row gets a button to stop being asked about that setting.
  let { changes, onmute }: { changes: Change[]; onmute?: (change: Change) => void } = $props();
</script>

<!-- One grid that every row shares: name, old, arrow, new, and the mute button if there is one. -->
<ul
  class="grid gap-x-1.5 divide-y divide-border text-xs {onmute
    ? 'grid-cols-[minmax(0,1fr)_auto_auto_auto_auto]'
    : 'grid-cols-[minmax(0,1fr)_auto_auto_auto]'}"
>
  {#each changes as change (change.file + change.section + change.key)}
    <li class="col-span-full grid grid-cols-subgrid items-center px-2.5 py-2">
      <span class="min-w-0 truncate pr-2" title="{change.section} / {change.key}">{settingLabel(change.key)}</span>
      <FromTo keys={isBind(change)} name={change.key} from={change.from} to={change.to} />
      {#if onmute}
        <button
          class="ml-1 shrink-0 rounded p-0.5 text-faint hover:text-foreground"
          title="Never ask about this setting again"
          aria-label="Don't ask about {settingLabel(change.key)} again"
          onclick={() => onmute(change)}
        >
          <BellOff class="size-3.5" />
        </button>
      {/if}
    </li>
  {/each}
</ul>
