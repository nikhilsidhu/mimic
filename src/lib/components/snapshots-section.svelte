<!-- The settings as they were before each apply, newest first, to go back to. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { Button } from "$lib/components/ui/button";
  import * as api from "$lib/api";

  let { onmessage }: { onmessage: (text: string, failed: boolean) => void } = $props();

  let snapshots = $state<api.SnapshotView[]>([]);
  let busy = $state<string | null>(null);

  const refresh = async () => (snapshots = await api.getSnapshots());

  onMount(() => {
    refresh();
    return api.onViewChanged(refresh);
  });

  async function restore(snapshot: api.SnapshotView) {
    if (busy) return;
    busy = snapshot.id;
    try {
      onmessage(await api.restoreSnapshot(snapshot.id), false);
    } catch (err) {
      onmessage(String(err), true);
    } finally {
      busy = null;
      refresh();
    }
  }

  const when = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });
  /** "before applying 'Main'" reads better as "Before applying Main". */
  const reason = (text: string) => text.replace(/'/g, "").replace(/^\w/, (c) => c.toUpperCase());
</script>

<header class="pt-2">
  <h2 class="text-lg font-semibold tracking-tight">History</h2>
  <p class="text-sm text-muted-foreground">
    Settings as they were before each change.
  </p>
</header>

<section class="overflow-hidden rounded-lg border border-border">
  {#each snapshots as snapshot, index (snapshot.id)}
    <div class="flex min-h-14 items-center gap-3 px-4 py-2.5" class:border-t={index > 0}>
      <div class="min-w-0 flex-1">
        <p class="truncate text-sm font-medium">{reason(snapshot.reason)}</p>
        <p class="truncate text-xs text-faint">
          {when.format(new Date(snapshot.taken))}
          {#if snapshot.account}· {snapshot.account}{/if}
          · {snapshot.settings} settings
        </p>
      </div>
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
  {:else}
    <p class="px-4 py-10 text-center text-sm text-muted-foreground">None yet.</p>
  {/each}
</section>
