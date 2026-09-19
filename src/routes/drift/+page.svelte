<!-- Asks what to do about settings the user changed, after a game or at login. -->
<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import { Button } from "$lib/components/ui/button";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import * as api from "$lib/api";

  let drift = $state<api.Drift | null>(null);
  let busy = $state(false);
  let failure = $state<string | null>(null);

  const close = () => getCurrentWindow().hide();

  async function refresh() {
    failure = null;
    drift = await api.getDrift();
    // Whatever changed may have been changed back in the meantime.
    if (!drift) close();
  }

  onMount(() => {
    refresh();
    return api.onDriftChanged(refresh);
  });

  async function choose(choice: api.DriftChoice) {
    if (busy) return;
    busy = true;
    try {
      const said = await api.resolveDrift(choice);
      close();
      api.notify(said);
    } catch (err) {
      failure = String(err);
    } finally {
      busy = false;
    }
  }

  /** `[<Unbound>]` and the empty string both mean no key. */
  const shown = (value: string | null) => (!value || value === "[<Unbound>]" ? "none" : value);
  /** `evtCastSpell1` reads better as `CastSpell1`. */
  const label = (change: api.Change) => change.key.replace(/^evn?t/, "");
</script>

<main class="flex h-screen flex-col gap-3 border border-border bg-popover p-4 text-popover-foreground">
  <div>
    <p class="text-sm font-medium">
      {drift?.changes.length ?? 0}
      {drift?.changes.length === 1 ? "setting" : "settings"} changed
    </p>
    <p class="text-xs text-muted-foreground">
      {#if drift?.profile}
        Save to <span class="text-foreground">{drift.profile}</span> and your other accounts get it too.
      {:else if drift?.champion}
        This account is not on a profile, but it can be kept for {drift.champion.name}.
      {:else}
        This account is not on a profile, so there is nowhere to save it.
      {/if}
    </p>
  </div>

  <ScrollArea class="min-h-0 flex-1 rounded-md border border-border">
    <ul class="divide-y divide-border text-xs">
      {#each drift?.changes ?? [] as change (change.file + change.section + change.key)}
        <li class="flex items-center gap-2 px-2.5 py-1.5">
          <span class="min-w-0 flex-1 truncate" title="{change.section} / {change.key}">{label(change)}</span>
          <span class="shrink-0 text-muted-foreground">{shown(change.from)}</span>
          <ArrowRight class="size-3 shrink-0 text-muted-foreground" />
          <span class="shrink-0 font-medium">{shown(change.to)}</span>
        </li>
      {/each}
    </ul>
  </ScrollArea>

  {#if failure}
    <p class="text-xs text-destructive">{failure}</p>
  {/if}

  <!-- Where the changes go, then what else can be done with them. -->
  <div class="flex flex-col gap-2">
    {#if drift?.profile || drift?.champion}
      <div class="flex gap-2">
        {#if drift.profile}
          <Button class="min-w-0 flex-1" disabled={busy} onclick={() => choose("saveToProfile")}>
            <span class="truncate">Save to {drift.profile}</span>
          </Button>
        {/if}
        {#if drift.champion}
          <Button
            class="min-w-0 flex-1"
            variant={drift.profile ? "secondary" : "default"}
            disabled={busy}
            onclick={() => choose("saveToChampion")}
            title="Only used when you play {drift.champion.name}"
          >
            <span class="truncate">Only for {drift.champion.name}</span>
          </Button>
        {/if}
      </div>
    {/if}
    <div class="flex gap-2">
      <Button class="flex-1" variant="ghost" disabled={busy} onclick={() => choose("keepHere")}>Keep here only</Button>
      <Button class="flex-1" variant="ghost" disabled={busy} onclick={() => choose("revert")}>Revert</Button>
    </div>
  </div>
</main>
