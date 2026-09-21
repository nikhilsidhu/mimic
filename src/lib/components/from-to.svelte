<!-- A value changing: the old one, an arrow, the new one. It is three cells of the list's own
     grid, not a box of its own, so that the list sizes the columns like a table: down it the old
     values end at the arrow, the arrows share a column, the new values start together, and the
     widest of them reaches the edge. The row it is in must be a subgrid with three columns
     for these. -->
<script lang="ts">
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import Bind from "$lib/components/bind.svelte";
  import { valueLabel } from "$lib/binds";

  type Props = {
    /** Whether these are keybinds, drawn as keys, or plain values. */
    keys: boolean;
    /** The setting's internal name, by which a plain value is put into words: 1 as "On". */
    name: string;
    /** The old value; `undefined` when it is not known, which leaves its cell empty. */
    from?: string | null;
    to: string | null;
  };
  let { keys, name, from, to }: Props = $props();

  const said = (value: string | null) => (value === null ? "none" : valueLabel(name, value));
</script>

<span class="contents">
  <span class="flex justify-end">
    {#if from !== undefined}
      {#if keys}<Bind value={from} />{:else}<span class="text-muted-foreground">{said(from)}</span>{/if}
    {/if}
  </span>
  <span class="flex w-3 justify-center">
    {#if from !== undefined}<ArrowRight class="size-3 text-faint" />{/if}
  </span>
  <span class="flex justify-start">
    {#if keys}<Bind value={to} />{:else}<span class="font-medium">{said(to)}</span>{/if}
  </span>
</span>
