<!-- The settings the user changed, and the choice of where they go. Shown by itself in the
     prompt after a game, and inside the tray panel when asked for from there. -->
<script lang="ts">
  import { onMount } from "svelte";
  import X from "@lucide/svelte/icons/x";
  import ChangeList from "$lib/components/change-list.svelte";
  import { Button } from "$lib/components/ui/button";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import * as api from "$lib/api";

  type Props = {
    /** Decide later, or nothing is left to decide: whoever shows this puts it away. */
    onclose: () => void;
    /** The changes were settled; `said` is the sentence about it. */
    ondone: (said: string) => void;
  };
  let { onclose, ondone }: Props = $props();

  let drift = $state<api.Drift | null>(null);
  let busy = $state(false);
  let failure = $state<string | null>(null);

  type Option = { choice: api.DriftChoice; title: string; says: string };

  // What can be done with the changes, the most likely first: after a reset by Riot that is
  // putting things back, otherwise it is saving them. `says` is the tooltip; the titles have
  // to be clear without it.
  const choices = $derived.by((): Option[] => {
    if (!drift) return [];
    const { profile, champion, reset } = drift;
    const options: Option[] = [];
    if (profile) {
      options.push({
        choice: "saveToProfile",
        title: `Save to ${profile}`,
        says: `Every account that uses ${profile} gets these changes.`,
      });
    }
    if (champion) {
      options.push({
        choice: "saveToChampion",
        title: `Save for ${champion.name} only`,
        says: `Used whenever you play ${champion.name}, and taken off afterwards.`,
      });
    }
    options.push({
      choice: "keepHere",
      title: "Keep on this account only",
      says: profile ? `The changes stay on this account. ${profile} is not changed.` : "The changes stay on this account.",
    });
    const revert: Option = {
      choice: "revert",
      title: reset ? "Restore my settings" : "Undo changes",
      says: "Puts everything back the way it was.",
    };
    if (reset) options.unshift(revert);
    else options.push(revert);
    return options;
  });

  async function refresh() {
    failure = null;
    drift = await api.getDrift();
    // Whatever changed may have been changed back in the meantime.
    if (!drift) onclose();
  }

  onMount(() => {
    refresh();
    return api.onDriftChanged(refresh);
  });

  async function act(action: () => Promise<void>) {
    if (busy) return;
    busy = true;
    try {
      await action();
    } catch (err) {
      failure = String(err);
    } finally {
      busy = false;
    }
  }

  const choose = (choice: api.DriftChoice) => act(async () => ondone(await api.resolveDrift(choice)));
  // Muting the last row leaves nothing to ask; refresh then closes this.
  const mute = (change: api.Change) =>
    act(async () => {
      await api.muteSetting(api.muteId(change));
      await refresh();
    });
</script>

<svelte:window onkeydown={(event) => event.key === "Escape" && onclose()} />

<div class="flex min-h-0 flex-1 flex-col gap-3">
  <div class="relative">
    <!-- Decide later: the changes stay, and the tray panel offers to review them. -->
    <Button
      class="absolute -top-1.5 -right-1.5 text-faint"
      variant="ghost"
      size="icon-sm"
      onclick={onclose}
      aria-label="Decide later"
      title="Decide later"
    >
      <X />
    </Button>
    <p class="pr-8 text-sm font-medium">
      {#if drift?.reset}
        Riot reset your settings
      {:else}
        {drift?.changes.length ?? 0}
        {drift?.changes.length === 1 ? "setting" : "settings"} changed
      {/if}
    </p>
    <p class="text-xs text-muted-foreground">
      {#if drift?.reset}
        A patch put {drift.changes.length === 1 ? "1 setting" : `${drift.changes.length} settings`} back to Riot's
        defaults. Restore puts yours back.
      {:else}
        Where should {drift?.changes.length === 1 ? "it" : "they"} go?
      {/if}
    </p>
  </div>

  <!-- As tall as its rows, so the box ends where the list does; with many it shrinks and scrolls. -->
  <ScrollArea class="min-h-0 shrink rounded-md border border-border">
    <ChangeList changes={drift?.changes ?? []} onmute={mute} />
  </ScrollArea>

  {#if failure}
    <p class="text-xs text-destructive">{failure}</p>
  {/if}

  <div class="mt-auto flex flex-col gap-1">
    {#each choices as option, index (option.choice)}
      <button
        class="rounded-md border px-2.5 py-1.5 text-left text-sm font-medium transition-colors hover:bg-accent disabled:opacity-50"
        class:border-border={index === 0}
        class:border-transparent={index > 0}
        class:bg-accent={index === 0}
        title={option.says}
        disabled={busy}
        onclick={() => choose(option.choice)}
      >
        {option.title}
      </button>
    {/each}
  </div>
</div>
