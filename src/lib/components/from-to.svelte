<!-- A value changing: the old one, an arrow, the new one, kept together and pushed to the right,
     so that down a list the new values end on one line and the old ones reach out to the left
     as far as they need. -->
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

<span class="flex shrink-0 items-center justify-end gap-1.5">
  <span class="flex justify-end">
    {#if from !== undefined}
      {#if keys}<Bind value={from} />{:else}<span class="text-muted-foreground">{said(from)}</span>{/if}
    {/if}
  </span>
  {#if from !== undefined}<ArrowRight class="size-3 shrink-0 text-faint" />{/if}
  <span class="flex justify-start">
    {#if keys}<Bind value={to} />{:else}<span class="font-medium">{said(to)}</span>{/if}
  </span>
</span>
