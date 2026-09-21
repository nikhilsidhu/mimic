<!-- The manager: the account in use, then a section for everything mimic keeps. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Power from "@lucide/svelte/icons/power";
  import AccountsSection from "$lib/components/accounts-section.svelte";
  import Avatar from "$lib/components/avatar.svelte";
  import ChampionsSection from "$lib/components/champions-section.svelte";
  import Confirm from "$lib/components/confirm.svelte";
  import ProfilesSection from "$lib/components/profiles-section.svelte";
  import SettingsSection from "$lib/components/settings-section.svelte";
  import SnapshotsSection from "$lib/components/snapshots-section.svelte";
  import Titlebar from "$lib/components/titlebar.svelte";
  import Toast from "$lib/components/toast.svelte";
  import { Button } from "$lib/components/ui/button";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import * as api from "$lib/api";
  import type { Report } from "$lib/attempt";
  import { DOT_LIVE, DOT_OFF } from "$lib/tones";

  let view = $state<api.View | null>(null);
  let message = $state<{ text: string; failed: boolean } | null>(null);
  // Whether the header shows every account, not just the one in use.
  let showAccounts = $state(false);
  /** Whether the header is asking before it quits. */
  let quitting = $state(false);

  const refresh = async () => (view = await api.getView());
  /** What a section's action had to say goes into the toast. */
  const report: Report = (text, failed) => (message = { text, failed });

  onMount(() => {
    refresh();
    return api.onViewChanged(refresh);
  });

  let page = $state<HTMLElement | null>(null);
  /** The title bar, which the page sits under. */
  const TITLEBAR = 32;
  // Once, when there is first something to show: the window is made as tall as the page, so that
  // it opens fitting what it holds. Not again, or it would fight whoever resizes it.
  let fitted = false;
  $effect(() => {
    if (!view || !page || fitted) return;
    fitted = true;
    tick().then(() => requestAnimationFrame(() => page && api.fitManager(page.scrollHeight + TITLEBAR)));
  });
</script>

<div class="flex h-screen flex-col bg-background">
  <Titlebar />

  <ScrollArea class="fade-edges min-h-0 flex-1">
    <main bind:this={page} class="mx-auto flex max-w-4xl flex-col gap-6 px-6 pt-6 pb-10">
      <header class="flex items-center gap-4">
        <Avatar account={view?.account} size="lg" />
        <div class="min-w-0 flex-1">
          <h1 class="truncate text-lg font-semibold tracking-tight">{view?.account?.name ?? "mimic"}</h1>
          <p class="flex min-w-0 items-center gap-2 text-sm text-muted-foreground">
            <span class="size-1.5 shrink-0 rounded-full {view?.connected ? DOT_LIVE : DOT_OFF}"></span>
            <span class="truncate">
              {#if !view}
                Starting…
              {:else if view.connected}
                {view.activity ?? "Connected"}
              {:else}
                {view.status}
              {/if}
            </span>
          </p>
        </div>
        {#if quitting}
          <!-- Quitting stops auto-apply and champion settings, so it asks first, as the tray does. -->
          <p class="text-sm">Quit mimic?</p>
          <Confirm action="Quit" onconfirm={api.quit} oncancel={() => (quitting = false)} />
        {:else}
          <!-- The other accounts fold out of the one in use. -->
          <Button variant="ghost" size="sm" aria-expanded={showAccounts} onclick={() => (showAccounts = !showAccounts)}>
            Accounts
            <ChevronRight class="transition-transform {showAccounts ? 'rotate-90' : ''}" />
          </Button>
          <!-- Closing the window only hides it; this is how mimic is stopped from here. -->
          <Button class="hover-danger text-muted-foreground" variant="ghost" size="icon" onclick={() => (quitting = true)} aria-label="Quit mimic" title="Quit mimic">
            <Power />
          </Button>
        {/if}
      </header>
      {#if showAccounts}
        <AccountsSection onmessage={report} />
      {/if}

      <ProfilesSection {view} onmessage={report} onchanged={refresh} />
      <ChampionsSection {view} onmessage={report} onchanged={refresh} />
      <SettingsSection onmessage={report} />
      <SnapshotsSection onmessage={report} />
    </main>
  </ScrollArea>
  <Toast bind:message />
</div>
