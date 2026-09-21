<!-- One row of the Settings list: what it is, a line about it if that helps, and its control.
     As a `label`, clicking anywhere on the row works the control. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = { title: string; label?: boolean; description?: Snippet; children: Snippet };
  let { title, label = false, description, children }: Props = $props();
</script>

<svelte:element
  this={label ? "label" : "div"}
  class="flex min-h-14 items-center gap-3 border-t border-border px-4 py-2.5 first:border-t-0"
>
  <div class="min-w-0 flex-1">
    <p class="text-sm font-medium">{title}</p>
    {#if description}{@render description()}{/if}
  </div>
  {@render children()}
</svelte:element>
