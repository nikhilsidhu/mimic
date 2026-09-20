<script lang="ts">
  import { onMount } from "svelte";
  import Check from "@lucide/svelte/icons/check";
  import Copy from "@lucide/svelte/icons/copy";
  import FileText from "@lucide/svelte/icons/file-text";
  import LoaderCircle from "@lucide/svelte/icons/loader-circle";
  import Power from "@lucide/svelte/icons/power";
  import Settings from "@lucide/svelte/icons/settings";
  import Undo2 from "@lucide/svelte/icons/undo-2";
  import Avatar from "$lib/components/avatar.svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import { Separator } from "$lib/components/ui/separator";
  import { Switch } from "$lib/components/ui/switch";
  import * as api from "$lib/api";

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

  async function save(event: SubmitEvent) {
    event.preventDefault();
    await run("save", () => api.saveCurrent(newName));
    if (!message?.failed) newName = "";
  }
</script>

<main class="flex h-screen flex-col overflow-hidden border border-border bg-background text-xs select-none">
  <header class="flex items-center gap-2.5 px-3 py-2.5">
    <Avatar account={view?.account} />
    <div class="min-w-0 flex-1">
      <p class="text-[0.625rem] font-medium tracking-wide text-muted-foreground">mimic</p>
      <p class="truncate text-sm font-medium" class:text-muted-foreground={!view?.connected}>
        {view?.status ?? "Starting…"}
      </p>
    </div>
    {#if view?.connected}
      {#if view.phase && view.phase !== "None"}
        <Badge variant="secondary">{view.phase}</Badge>
      {/if}
      <Button variant="ghost" size="icon" onclick={copy} aria-label="Copy Riot ID" title="Copy Riot ID">
        {#if copied}<Check class="text-emerald-500" />{:else}<Copy />{/if}
      </Button>
    {/if}
  </header>

  {#if view?.activeOverlay}
    <div class="flex items-center gap-2 px-3 pb-2.5">
      <img src={view.activeOverlay.icon} alt="" class="size-5 rounded-sm" />
      <span class="min-w-0 flex-1 truncate">
        <span class="text-foreground">{view.activeOverlay.name}</span>
        <span class="text-muted-foreground">settings are on for this game</span>
      </span>
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
              <Check class="size-3.5" />
            {/if}
          </span>
          <span class="min-w-0 flex-1 truncate text-sm">{profile.name}</span>
          <span class="shrink-0 text-muted-foreground">
            {view?.pending === profile.name ? "at next login" : `${profile.settings} settings`}
          </span>
        </button>
      {:else}
        <p class="px-2 py-6 text-center text-muted-foreground">
          No profiles yet.<br />Log into League, then save your current settings below.
        </p>
      {/each}
    </div>
  </ScrollArea>

  <Separator />

  <form class="flex gap-1.5 p-2" onsubmit={save}>
    <Input
      bind:value={newName}
      placeholder="Save current settings as…"
      maxlength={40}
      disabled={!view?.connected || busy !== null}
    />
    <Button type="submit" disabled={!view?.connected || busy !== null || !newName.trim()}>
      {#if busy === "save"}<LoaderCircle class="animate-spin" />{/if}
      Save
    </Button>
  </form>

  {#if message}
    <p class="px-3 pb-2 leading-snug" class:text-destructive={message.failed} class:text-muted-foreground={!message.failed}>
      {message.text}
    </p>
  {/if}

  <Separator />

  <footer class="flex items-center gap-1 p-1.5">
    <Button
      variant="ghost"
      size="sm"
      disabled={!view?.connected || busy !== null}
      onclick={() => run("undo", api.undoLast)}
    >
      {#if busy === "undo"}<LoaderCircle class="animate-spin" />{:else}<Undo2 />{/if}
      Undo last apply
    </Button>
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
