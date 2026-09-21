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

  type Option = { choice: api.DriftChoice; title: string; says: string; primary: boolean };

  // What can be done with the changes, the most likely first. After a reset by Riot that
  // is putting things back; otherwise it is saving them.
  const choices = $derived.by((): Option[] => {
    if (!drift) return [];
    const { profile, champion, reset } = drift;
    const options: Option[] = [];
    if (profile) {
      options.push({
        choice: "saveToProfile",
        title: `Save to ${profile}`,
        says: "all your accounts",
        primary: false,
      });
    }
    if (champion) {
      options.push({
        choice: "saveToChampion",
        title: `Only for ${champion.name}`,
        says: "just this champion",
        primary: false,
      });
    }
    options.push({
      choice: "keepHere",
      title: "Only this account",
      says: "profile untouched",
      primary: false,
    });
    const revert: Option = {
      choice: "revert",
      title: reset ? "Restore my settings" : "Revert",
      says: "undo the changes",
      primary: false,
    };
    if (reset) options.unshift(revert);
    else options.push(revert);
    options[0].primary = true;
    return options;
  });

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
      {:else}
        Where should {drift?.changes.length === 1 ? "it" : "they"} go?
      {/if}
    </p>
  </div>

  <ScrollArea class="min-h-0 flex-1 rounded-md border border-border">
    <ChangeList changes={drift?.changes ?? []} onmute={mute} />
  </ScrollArea>

  {#if failure}
    <p class="text-xs text-destructive">{failure}</p>
  {/if}

  <!-- Where the changes go. Each choice says what it does, as the names alone did not. -->
  <div class="flex flex-col gap-1">
    {#each choices as option (option.choice)}
      <button
        class="flex items-baseline gap-3 rounded-md border px-2.5 py-1.5 text-left transition-colors hover:bg-accent disabled:opacity-50"
        class:border-border={option.primary}
        class:border-transparent={!option.primary}
        class:bg-accent={option.primary}
        disabled={busy}
        onclick={() => choose(option.choice)}
      >
        <span class="min-w-0 flex-1 truncate text-sm font-medium">{option.title}</span>
        <span class="shrink-0 text-xs text-muted-foreground">{option.says}</span>
      </button>
    {/each}
  </div>
</main>
