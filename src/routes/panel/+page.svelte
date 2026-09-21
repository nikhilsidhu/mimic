<script lang="ts">
  import { onMount } from "svelte";
  import Check from "@lucide/svelte/icons/check";
  import Copy from "@lucide/svelte/icons/copy";
  import LoaderCircle from "@lucide/svelte/icons/loader-circle";
  import Power from "@lucide/svelte/icons/power";
  import Save from "@lucide/svelte/icons/save";
  import Settings from "@lucide/svelte/icons/settings";
  import Avatar from "$lib/components/avatar.svelte";
  import ChangesReview from "$lib/components/changes-review.svelte";
  import FlareImage from "$lib/components/flare-image.svelte";
  import Confirm from "$lib/components/confirm.svelte";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import { Separator } from "$lib/components/ui/separator";
  import Status from "$lib/components/status.svelte";
  import * as api from "$lib/api";
  import { DOT_LIVE, DOT_OFF, WAITING } from "$lib/tones";

  const MESSAGE_MS = 5000;

  let view = $state<api.View | null>(null);
  let newName = $state("");
  /** What is running: a profile id, "save" or "undo". One action at a time. */
  let busy = $state<string | null>(null);
  let message = $state<{ text: string; failed: boolean } | null>(null);
  let copied = $state(false);
  let messageTimer: ReturnType<typeof setTimeout> | undefined;

  const refresh = async () => (view = await api.getView());
  /** The profile this account is on, which is what auto-apply would apply. */
  const activeProfile = $derived(view?.connected ? view.profiles.find((profile) => profile.active) : undefined);

  /** For buttons that open something else: the panel gets out of the way, like a menu. */
  function leaveFor(action: () => Promise<void>) {
    api.dismiss();
    action().catch((err) => api.notify(String(err)));
  }

  onMount(() => {
    refresh();
    const stop = api.onViewChanged(refresh);
    // While reviewing, Esc goes back to the panel; otherwise it closes it.
    const onKey = (event: KeyboardEvent) => event.key === "Escape" && !reviewing && api.dismiss();
    window.addEventListener("keydown", onKey);
    return () => {
      stop();
      window.removeEventListener("keydown", onKey);
    };
  });

  function say(text: string, failed = false) {
    message = { text, failed };
    clearTimeout(messageTimer);
    messageTimer = setTimeout(() => (message = null), MESSAGE_MS);
  }

  async function run(what: string, action: () => Promise<string>) {
    if (busy) return;
    busy = what;
    try {
      say(await action());
    } catch (err) {
      say(String(err), true);
    } finally {
      busy = null;
      refresh();
    }
  }

  async function copy() {
    try {
      await api.copyRiotId();
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch (err) {
      say(String(err), true);
    }
  }

  /** Saving: not asked for, choosing where to, or naming a new profile. */
  let saving = $state<"closed" | "menu" | "new">("closed");
  /** Whether the panel shows the settings that changed in place of its usual content. */
  let reviewing = $state(false);
  /** Whether the footer is asking before it quits. */
  let quitting = $state(false);
  const saveRow = "rounded-md px-2 py-1.5 text-left transition-colors pressable disabled:opacity-50";

  async function saveTo(action: () => Promise<string>) {
    await run("save", action);
    if (!message?.failed) saving = "closed";
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    await saveTo(() => api.saveCurrent(newName));
    if (!message?.failed) newName = "";
  }
</script>

<main class="flex h-screen flex-col overflow-hidden border border-border bg-background text-xs select-none">
  {#if reviewing}
    <!-- The changed settings are gone through right here, where Review was clicked. -->
    <div class="flex min-h-0 flex-1 flex-col p-3">
      <ChangesReview
        onclose={() => (reviewing = false)}
        ondone={(said) => {
          reviewing = false;
          say(said);
          refresh();
        }}
      />
    </div>
  {:else}
  <header class="flex items-start gap-2.5 px-3 py-2.5">
    <Avatar account={view?.account} />
    <div class="min-w-0 flex-1">
      <!-- The copy button shares the name's line, so that the line under it has the whole width:
           "Ranked Solo/Duo · Champ select" did not fit next to a button. -->
      <div class="flex h-5 items-center gap-1">
        <p class="min-w-0 flex-1 truncate text-sm font-medium" class:text-muted-foreground={!view?.connected}>
          {view?.account?.name ?? "mimic"}
        </p>
        {#if view?.account}
          <Button class="-mr-1" variant="ghost" size="icon-sm" onclick={copy} aria-label="Copy Riot ID" title="Copy Riot ID">
            {#if copied}<Check class="text-live" />{:else}<Copy />{/if}
          </Button>
        {/if}
      </div>
      <!-- What the account is doing, or why there is none. The account itself stays put while
           League is closed. Wrapped rather than cut off when a queue's name is long. -->
      <p class="flex items-start gap-1.5 pt-0.5 text-xs leading-snug text-muted-foreground">
        <span class="mt-[0.3rem] size-1.5 shrink-0 rounded-full {view?.connected ? DOT_LIVE : DOT_OFF}"></span>
        <span class="min-w-0">
          {#if !view}
            Starting…
          {:else if view.connected}
            {view.activity ?? "Connected"}
          {:else}
            {view.status}
          {/if}
        </span>
      </p>
      <!-- The champion whose settings are on, under the name with the rest of what is said about
           the account. -->
      {#if view?.activeOverlay}
        <div class="flex items-center gap-1.5 pt-1.5" title="{view.activeOverlay.name}'s own settings are on for this game">
          <FlareImage src={view.activeOverlay.icon} class="size-4 rounded-sm" />
          <span class="min-w-0 truncate">{view.activeOverlay.name}</span>
          <Status>active</Status>
        </div>
      {/if}
    </div>
  </header>

  {#if view?.changed}
    <!-- The prompt about them was closed without a decision; this brings it back. -->
    <div class="flex items-center gap-2 px-3 pb-2.5">
      <span class="min-w-0 flex-1 truncate">
        {view.changed === 1 ? "1 setting" : `${view.changed} settings`} changed
      </span>
      <Button variant="outline" size="sm" onclick={() => (reviewing = true)}>Review</Button>
    </div>
  {/if}

  <Separator />

  {#if saving === "menu"}
    <!-- Where this account's current settings go. It takes the list's place while it is open: the
         panel has no room for both, and the bar below must stay in view. -->
    <p class="px-3 pt-2.5 pb-1 eyebrow">Save settings to</p>
    <div class="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-1.5 pb-1.5">
      {#if activeProfile}
        <button class={saveRow} disabled={busy !== null} onclick={() => saveTo(() => api.updateProfile(activeProfile.id))}>
          <span class="block text-sm">{activeProfile.name}</span>
          <span class="block text-muted-foreground">Replaces its settings.</span>
        </button>
      {/if}
      <button class={saveRow} onclick={() => (saving = "new")}>
        <span class="block text-sm">A new profile</span>
        <span class="block text-muted-foreground">As a separate profile.</span>
      </button>
      <button class={saveRow} onclick={() => leaveFor(api.addChampion)}>
        <span class="block text-sm">A champion</span>
        <span class="block text-muted-foreground">Pick the champion in the manager.</span>
      </button>
    </div>
  {:else}
  <p class="px-3 pt-2.5 pb-1 eyebrow">Profiles</p>
  <ScrollArea class="min-h-0 flex-1">
    <div class="flex flex-col gap-0.5 px-1.5 pb-1.5">
      {#each view?.profiles ?? [] as profile (profile.id)}
        <button
          class="flex items-center gap-2 rounded-md px-2 py-1.5 text-left pressable disabled:opacity-50"
          disabled={busy !== null}
          onclick={() => run(profile.id, () => api.applyProfile(profile.id))}
          title={view?.connected ? `Apply ${profile.name}` : `Apply ${profile.name} at the next login`}
        >
          <span class="flex size-4 shrink-0 items-center justify-center">
            {#if busy === profile.id}
              <LoaderCircle class="size-3.5 animate-spin" />
            {:else if profile.active}
              <span class="size-1.5 rounded-full {DOT_LIVE}" title="In use on this account"></span>
            {/if}
          </span>
          <span class="min-w-0 flex-1 truncate text-sm">{profile.name}</span>
          {#if view?.pending === profile.name}
            <span class="shrink-0 {WAITING}">at next login</span>
          {:else}
            <span class="shrink-0 text-faint">{profile.settings} settings</span>
          {/if}
        </button>
      {:else}
        <p class="px-2 py-6 text-center text-muted-foreground">
          No profiles yet.<br />Log into League, then save your current settings below.
        </p>
      {/each}
    </div>
  </ScrollArea>
  {/if}

  <Separator />

  {#if saving === "new"}
    <form class="flex gap-1.5 p-2" onsubmit={save}>
      <Input bind:value={newName} placeholder="Profile name" maxlength={40} disabled={busy !== null} />
      <Button type="submit" disabled={busy !== null || !newName.trim()}>
        {#if busy === "save"}<LoaderCircle class="animate-spin" />{/if}
        Save
      </Button>
      <Button type="button" variant="ghost" onclick={() => (saving = "closed")}>Cancel</Button>
    </form>
  {/if}

  {#if message}
    <p class="px-3 pb-2 leading-snug" class:text-destructive={message.failed} class:text-muted-foreground={!message.failed}>
      {message.text}
    </p>
  {/if}

  <Separator />

  <footer class="flex items-center gap-1 p-1.5">
    {#if quitting}
      <!-- Quitting stops the auto-apply and the champion settings, so it asks first. -->
      <p class="min-w-0 flex-1 truncate px-1.5">Quit mimic?</p>
      <Confirm action="Quit" onconfirm={api.quit} oncancel={() => (quitting = false)} />
    {:else}
    <!-- Opens the choice of where this account's current settings go, shown above. -->
    <Button
      variant="ghost"
      size="sm"
      disabled={!view?.connected || busy !== null}
      aria-expanded={saving !== "closed"}
      onclick={() => (saving = saving === "closed" ? "menu" : "closed")}
    >
      <Save />Save settings
    </Button>
    <span class="flex-1"></span>
    <Button variant="ghost" size="icon" onclick={() => leaveFor(api.openManager)} aria-label="Open manager" title="Open manager">
      <Settings />
    </Button>
    <Button variant="ghost" size="icon" onclick={() => (quitting = true)} aria-label="Quit mimic" title="Quit mimic">
      <Power />
    </Button>
    {/if}
  </footer>
  {/if}
</main>
