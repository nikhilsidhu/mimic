<!-- The settings as they were before each apply, newest first, to go back to. -->
<script lang="ts">
  import { onMount } from "svelte";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Undo2 from "@lucide/svelte/icons/undo-2";
  import ChangeList from "$lib/components/change-list.svelte";
  import { Button } from "$lib/components/ui/button";
  import * as api from "$lib/api";
  import { attempt, type Report } from "$lib/attempt";

  let { onmessage }: { onmessage: Report } = $props();

  let snapshots = $state<api.SnapshotView[]>([]);
  let busy = $state<string | null>(null);
  /** History is reference material: folded away until asked for. */
  let expanded = $state(false);
  /** The entry whose changes are shown, by id. */
  let open = $state<string | null>(null);

  const refresh = async () => (snapshots = await api.getSnapshots());

  onMount(() => {
    refresh();
    return api.onViewChanged(refresh);
  });

  async function restore(snapshot: api.SnapshotView) {
    if (busy) return;
    busy = snapshot.id;
    await attempt(onmessage, () => api.restoreSnapshot(snapshot.id));
    busy = null;
    refresh();
  }

  async function undo() {
    if (busy) return;
    busy = "undo";
    await attempt(onmessage, api.undoLast);
    busy = null;
    refresh();
  }

  const when = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });
  /** "before applying 'Main'" reads better as "Before applying Main". */
  const reason = (text: string) => text.replace(/'/g, "").replace(/^\w/, (c) => c.toUpperCase());
</script>

<div class="flex items-end gap-3 pt-2">
  <button class="group flex min-w-0 flex-1 items-center gap-2 text-left" aria-expanded={expanded} onclick={() => (expanded = !expanded)}>
    <ChevronRight class="size-4 shrink-0 text-faint transition-transform {expanded ? 'rotate-90' : ''}" />
    <div class="min-w-0 flex-1">
      <h2 class="text-lg font-semibold tracking-tight">History</h2>
      <p class="text-sm text-muted-foreground">
        Every change mimic made, and what it altered.
        <span class="text-faint">{snapshots.length} {snapshots.length === 1 ? "entry" : "entries"}</span>
      </p>
    </div>
  </button>
  <!-- Goes back to before the newest change, without having to find it in the list. -->
  <Button variant="outline" size="sm" disabled={busy !== null || !snapshots[0]?.restorable} onclick={undo}>
    <Undo2 />Undo last change
  </Button>
</div>

{#if expanded}
<section class="list-box">
  {#each snapshots as snapshot, index (snapshot.id)}
    <div class="list-row" class:border-t={index > 0}>
      <button
        class="flex min-w-0 flex-1 items-center gap-2 text-left disabled:cursor-default"
        disabled={snapshot.changes.length === 0}
        onclick={() => (open = open === snapshot.id ? null : snapshot.id)}
      >
        <ChevronRight
          class="size-3.5 shrink-0 text-faint transition-transform {open === snapshot.id ? 'rotate-90' : ''} {snapshot.changes.length ? '' : 'invisible'}"
        />
        <div class="min-w-0 flex-1">
          <p class="truncate text-sm font-medium">{reason(snapshot.reason)}</p>
          <p class="truncate text-xs text-faint">
            {when.format(new Date(snapshot.taken))}
            {#if snapshot.account}· {snapshot.account}{/if}
            {#if snapshot.changes.length}· {snapshot.changes.length} changed{/if}
          </p>
        </div>
      </button>
      <Button
        variant="outline"
        size="sm"
        disabled={!snapshot.restorable || busy !== null}
        title={snapshot.restorable ? "Put the account back to this" : "Log into that account to restore this"}
        onclick={() => restore(snapshot)}
      >
        {busy === snapshot.id ? "Restoring…" : "Restore"}
      </Button>
    </div>
    {#if open === snapshot.id}
      <div class="border-t border-border bg-muted/30 px-4 py-1">
        <ChangeList changes={snapshot.changes} />
      </div>
    {/if}
  {:else}
    <p class="px-4 py-10 text-center text-sm text-muted-foreground">None yet.</p>
  {/each}
</section>
{/if}
