<!-- Which settings changed and how: binds as keycaps, everything else as plain values. -->
<script lang="ts">
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import BellOff from "@lucide/svelte/icons/bell-off";
  import Bind from "$lib/components/bind.svelte";
  import { isBind, settingLabel } from "$lib/binds";
  import type { Change } from "$lib/api";

  // With `onmute`, every row gets a button to stop being asked about that setting.
  let { changes, onmute }: { changes: Change[]; onmute?: (change: Change) => void } = $props();
</script>

<ul class="divide-y divide-border text-xs">
  {#each changes as change (change.file + change.section + change.key)}
    <li class="flex items-center gap-2 px-2.5 py-1.5">
      <span class="min-w-0 flex-1 truncate" title="{change.section} / {change.key}">{settingLabel(change.key)}</span>
      {#if isBind(change)}
        <Bind value={change.from} />
        <ArrowRight class="size-3 shrink-0 text-faint" />
        <Bind value={change.to} />
      {:else}
        <span class="shrink-0 text-muted-foreground">{change.from ?? "none"}</span>
        <ArrowRight class="size-3 shrink-0 text-faint" />
        <span class="shrink-0 font-medium">{change.to ?? "none"}</span>
      {/if}
      {#if onmute}
        <button
          class="shrink-0 rounded p-0.5 text-faint hover:text-foreground"
          title="Don't ask about this setting again. Changes to it are kept."
          aria-label="Don't ask about {settingLabel(change.key)} again"
          onclick={() => onmute(change)}
        >
          <BellOff class="size-3.5" />
        </button>
      {/if}
    </li>
  {/each}
</ul>
