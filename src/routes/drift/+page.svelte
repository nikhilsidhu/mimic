<!-- Asks what to do about settings the user changed, after a game or at login. -->
<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import X from "@lucide/svelte/icons/x";
  import ChangeList from "$lib/components/change-list.svelte";
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

  // Muting the last row leaves nothing to ask; refresh then closes the prompt.
  async function mute(change: api.Change) {
    if (busy) return;
    busy = true;
    try {
      await api.muteSetting(api.muteId(change));
      await refresh();
    } catch (err) {
      failure = String(err);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window onkeydown={(event) => event.key === "Escape" && close()} />

<main class="flex h-screen flex-col gap-3 border border-border bg-popover p-4 text-popover-foreground">
  <div class="relative">
    <!-- Decide later: the changes stay, and the tray panel offers to review them. -->
    <Button class="absolute -top-1.5 -right-1.5 text-faint" variant="ghost" size="icon-sm" onclick={close} aria-label="Decide later" title="Decide later">
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
      {:else if drift?.profile}
        Save to <span class="text-foreground">{drift.profile}</span> and your other accounts get it too.
      {:else if drift?.champion}
        This account is not on a profile, but it can be kept for {drift.champion.name}.
      {:else}
        This account is not on a profile, so there is nowhere to save it.
      {/if}
    </p>
  </div>

  <ScrollArea class="min-h-0 flex-1 rounded-md border border-border">
    <ChangeList changes={drift?.changes ?? []} onmute={mute} />
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
      <Button
        class="flex-1"
        variant="ghost"
        disabled={busy}
        onclick={() => choose("keepHere")}
        title="Keep the changes on this account. Your profile and other accounts stay as they are."
      >
        Only this account
      </Button>
      <Button class="flex-1" variant={drift?.reset ? "default" : "ghost"} disabled={busy} onclick={() => choose("revert")}>
        {drift?.reset ? "Restore my settings" : "Revert"}
      </Button>
    </div>
  </div>
</main>
