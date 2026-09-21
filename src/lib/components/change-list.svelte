<!-- Which settings changed and how: binds as keys, everything else as plain values. -->
<script lang="ts">
  import BellOff from "@lucide/svelte/icons/bell-off";
  import Undo2 from "@lucide/svelte/icons/undo-2";
  import FromTo from "$lib/components/from-to.svelte";
  import { isBind, settingLabel } from "$lib/binds";
  import { muteId, type Change } from "$lib/api";

  type Props = {
    changes: Change[];
    /** With this, every row gets a button to stop being asked about that setting. */
    onmute?: (change: Change) => void;
    /** Rows muted just now, by `muteId`: they stay in place, dimmed, with the way back on them. */
    muted?: string[];
    onunmute?: (change: Change) => void;
  };
  let { changes, onmute, muted = [], onunmute }: Props = $props();
</script>

<ul class="divide-y divide-border text-xs">
  {#each changes as change (change.file + change.section + change.key)}
    {@const off = muted.includes(muteId(change))}
    <li class="flex items-center gap-2 px-2.5 py-1.5">
      <span class="min-w-0 flex-1 truncate" class:line-through={off} class:opacity-50={off} title="{change.section} / {change.key}">
        {settingLabel(change.key)}
      </span>
      <span class="flex" class:opacity-40={off}>
        <FromTo keys={isBind(change)} name={change.key} from={change.from} to={change.to} />
      </span>
      {#if off}
        <button
          class="shrink-0 rounded p-0.5 text-muted-foreground hover:text-foreground"
          title="Undo: ask about this setting after all"
          aria-label="Ask about {settingLabel(change.key)} after all"
          onclick={() => onunmute?.(change)}
        >
          <Undo2 class="size-3.5" />
        </button>
      {:else if onmute}
        <button
          class="shrink-0 rounded p-0.5 text-faint hover:text-foreground"
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
