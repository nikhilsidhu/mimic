<!-- A value changing: the old one, an arrow, the new one. The columns are fixed, so that down a
     list the old values end at the same place, the arrows line up, and the new values start at
     the same place, whatever the widths of the keys. -->
<script lang="ts">
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import Bind from "$lib/components/bind.svelte";

  type Props = {
    /** Whether these are keybinds, drawn as keys, or plain values. */
    keys: boolean;
    /** The old value; `undefined` when it is not known, which leaves its column empty. */
    from?: string | null;
    to: string | null;
  };
  let { keys, from, to }: Props = $props();
</script>

<span class="grid shrink-0 grid-cols-[auto_0.75rem_4.75rem] items-center gap-1.5">
  <span class="flex justify-end">
    {#if from !== undefined}
      {#if keys}<Bind value={from} />{:else}<span class="text-muted-foreground">{from ?? "none"}</span>{/if}
    {/if}
  </span>
  {#if from !== undefined}<ArrowRight class="size-3 text-faint" />{:else}<span></span>{/if}
  <span class="flex justify-start">
    {#if keys}<Bind value={to} />{:else}<span class="font-medium">{to ?? "none"}</span>{/if}
  </span>
</span>
