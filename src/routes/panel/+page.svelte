<script lang="ts">
  import { onMount } from "svelte";
  import Check from "@lucide/svelte/icons/check";
  import Copy from "@lucide/svelte/icons/copy";
  import FileText from "@lucide/svelte/icons/file-text";
  import LoaderCircle from "@lucide/svelte/icons/loader-circle";
  import Power from "@lucide/svelte/icons/power";
  import Settings from "@lucide/svelte/icons/settings";
  import Avatar from "$lib/components/avatar.svelte";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import { Separator } from "$lib/components/ui/separator";
  import { Switch } from "$lib/components/ui/switch";
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

  async function setAutoApply(enabled: boolean) {
    try {
      await api.setAutoApply(enabled);
    } catch (err) {
      say(String(err), true);
    }
    refresh();
  }

  /** For buttons that open something else: the panel gets out of the way, like a menu. */
  function leaveFor(action: () => Promise<void>) {
    api.dismiss();
    action().catch((err) => api.notify(String(err)));
  }

  onMount(() => {
    refresh();
    const stop = api.onViewChanged(refresh);
    const onKey = (event: KeyboardEvent) => event.key === "Escape" && api.dismiss();
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
  const saveRow = "rounded-md px-2 py-1.5 text-left transition-colors hover:bg-accent disabled:opacity-50";

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
  <header class="flex items-center gap-2.5 px-3 py-2.5">
    <Avatar account={view?.account} />
    <div class="min-w-0 flex-1">
      <p class="truncate text-sm font-medium" class:text-muted-foreground={!view?.connected}>
        {view?.account?.name ?? "mimic"}
      </p>
      <!-- What the account is doing, or why there is none. The account itself stays put
           while League is closed. -->
      <p class="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
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
    {#if view?.account}
      <Button variant="ghost" size="icon" onclick={copy} aria-label="Copy Riot ID" title="Copy Riot ID">
        {#if copied}<Check class="text-emerald-500" />{:else}<Copy />{/if}
      </Button>
    {/if}
  </header>

  {#if view?.activeOverlay}
    <div class="flex items-center gap-2 px-3 pb-2.5" title="{view.activeOverlay.name}'s own settings are on for this game">
      <img src={view.activeOverlay.icon} alt="" class="size-5 rounded-sm" />
      <span class="min-w-0 truncate text-foreground">{view.activeOverlay.name}</span>
      <Status>on now</Status>
    </div>
  {/if}

  {#if view?.changed}
    <!-- The prompt about them was closed without a decision; this brings it back. -->
    <div class="flex items-center gap-2 px-3 pb-2.5">
      <span class="min-w-0 flex-1 truncate">
        {view.changed === 1 ? "1 setting" : `${view.changed} settings`} changed
      </span>
      <Button variant="secondary" size="sm" onclick={() => api.reviewChanges()}>Review</Button>
    </div>
  {/if}

  {#if activeProfile}
    <label class="flex items-center gap-2 px-3 pb-2.5 text-muted-foreground">
      <span class="min-w-0 flex-1 leading-snug">
        Apply <span class="text-foreground">{activeProfile.name}</span> whenever this account logs in
      </span>
      <Switch checked={view?.autoApply ?? false} onCheckedChange={setAutoApply} />
    </label>
  {/if}

  <Separator />

  <p class="px-3 pt-2.5 pb-1 text-[0.625rem] font-medium tracking-wide text-muted-foreground uppercase">Profiles</p>
  <ScrollArea class="min-h-0 flex-1">
    <div class="flex flex-col gap-0.5 px-1.5 pb-1.5">
      {#each view?.profiles ?? [] as profile (profile.id)}
        <button
          class="flex items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-accent disabled:opacity-50"
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

  <Separator />

  {#if saving === "closed"}
    <div class="p-2">
      <Button class="w-full" variant="secondary" disabled={!view?.connected || busy !== null} onclick={() => (saving = "menu")}>
        Save current settings…
      </Button>
    </div>
  {:else if saving === "menu"}
    <!-- Where this account's current settings go. -->
    <div class="flex flex-col gap-0.5 p-1.5">
      {#if activeProfile}
        <button class={saveRow} disabled={busy !== null} onclick={() => saveTo(() => api.updateProfile(activeProfile.id))}>
          <span class="block font-medium">Update {activeProfile.name}</span>
          <span class="block text-muted-foreground">Overwrite it with what is on this account now.</span>
        </button>
      {/if}
      <button class={saveRow} onclick={() => (saving = "new")}>
        <span class="block font-medium">New profile</span>
        <span class="block text-muted-foreground">Keep them under a name of their own.</span>
      </button>
      <button class={saveRow} onclick={() => leaveFor(api.addChampion)}>
        <span class="block font-medium">For a champion</span>
        <span class="block text-muted-foreground">Pick the champion in the manager.</span>
      </button>
      <Button variant="ghost" size="sm" onclick={() => (saving = "closed")}>Cancel</Button>
    </div>
  {:else}
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
    <span class="flex-1"></span>
    <Button variant="ghost" size="icon" onclick={() => leaveFor(api.openLogs)} aria-label="Open logs folder" title="Open logs folder">
      <FileText />
    </Button>
    <Button variant="ghost" size="icon" onclick={() => leaveFor(api.openManager)} aria-label="Open manager" title="Open manager">
      <Settings />
    </Button>
    <Button variant="ghost" size="icon" onclick={api.quit} aria-label="Quit mimic" title="Quit mimic">
      <Power />
    </Button>
  </footer>
</main>
