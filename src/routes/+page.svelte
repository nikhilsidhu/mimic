<script lang="ts">
  import { onMount, tick } from "svelte";
  import Check from "@lucide/svelte/icons/check";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import Titlebar from "$lib/components/titlebar.svelte";
  import * as api from "$lib/api";

  let view = $state<api.View | null>(null);
  /** The profile an action is running on. One action at a time. */
  let busy = $state<string | null>(null);
  let message = $state<{ text: string; failed: boolean } | null>(null);
  /** The row being renamed or asking to confirm a delete, if any. */
  let editing = $state<{ id: string; mode: "rename" | "delete"; name: string } | null>(null);
  let renameInput = $state<HTMLInputElement | null>(null);

  const refresh = async () => (view = await api.getView());

  onMount(() => {
    refresh();
    return api.onViewChanged(refresh);
  });

  async function run(id: string, action: () => Promise<string>) {
    if (busy) return;
    busy = id;
    try {
      message = { text: await action(), failed: false };
      editing = null;
    } catch (err) {
      message = { text: String(err), failed: true };
    } finally {
      busy = null;
      refresh();
    }
  }

  async function startRename(profile: api.ProfileView) {
    editing = { id: profile.id, mode: "rename", name: profile.name };
    await tick();
    renameInput?.select();
  }

  function submitRename(event: SubmitEvent) {
    event.preventDefault();
    if (editing) run(editing.id, () => api.renameProfile(editing!.id, editing!.name));
  }
</script>

<div class="flex h-screen flex-col bg-background">
  <Titlebar />

  <ScrollArea class="min-h-0 flex-1">
    <main class="mx-auto flex max-w-2xl flex-col gap-6 px-8 pt-6 pb-10">
      <header class="flex items-end justify-between gap-4">
        <div class="min-w-0">
          <h1 class="text-lg font-semibold tracking-tight">Profiles</h1>
          <p class="truncate text-sm text-muted-foreground">{view?.status ?? "Starting…"}</p>
        </div>
        {#if view?.phase && view.phase !== "None"}
          <Badge variant="secondary">{view.phase}</Badge>
        {/if}
      </header>

      <section class="overflow-hidden rounded-lg border border-border">
        {#each view?.profiles ?? [] as profile, index (profile.id)}
          <div class="group flex min-h-14 items-center gap-3 px-4 py-2.5" class:border-t={index > 0}>
            <span class="flex size-4 shrink-0 items-center justify-center">
              {#if profile.active}<Check class="size-4" />{/if}
            </span>

            {#if editing?.id === profile.id && editing.mode === "rename"}
              <form class="flex min-w-0 flex-1 items-center gap-2" onsubmit={submitRename}>
                <Input
                  bind:ref={renameInput}
                  bind:value={editing.name}
                  maxlength={40}
                  onkeydown={(event) => event.key === "Escape" && (editing = null)}
                />
                <Button type="submit" size="sm" disabled={busy !== null || !editing.name.trim()}>Rename</Button>
                <Button type="button" variant="ghost" size="sm" onclick={() => (editing = null)}>Cancel</Button>
              </form>
            {:else if editing?.id === profile.id && editing.mode === "delete"}
              <p class="min-w-0 flex-1 truncate text-sm">
                Delete <span class="font-medium">{profile.name}</span>? Accounts on it keep their settings.
              </p>
              <Button
                variant="destructive"
                size="sm"
                disabled={busy !== null}
                onclick={() => run(profile.id, () => api.deleteProfile(profile.id))}
              >
                Delete
              </Button>
              <Button variant="ghost" size="sm" onclick={() => (editing = null)}>Cancel</Button>
            {:else}
              <div class="min-w-0 flex-1">
                <p class="truncate text-sm font-medium">{profile.name}</p>
                <p class="text-xs text-muted-foreground">
                  {view?.pending === profile.name ? "Will be applied at the next login" : `${profile.settings} settings`}
                </p>
              </div>
              <div class="flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
                <Button variant="ghost" size="icon" onclick={() => startRename(profile)} aria-label="Rename" title="Rename">
                  <Pencil />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onclick={() => (editing = { id: profile.id, mode: "delete", name: profile.name })}
                  aria-label="Delete"
                  title="Delete"
                >
                  <Trash2 />
                </Button>
              </div>
              <Button
                variant="outline"
                size="sm"
                disabled={busy !== null}
                onclick={() => run(profile.id, () => api.applyProfile(profile.id))}
              >
                {busy === profile.id ? "Applying…" : "Apply"}
              </Button>
            {/if}
          </div>
        {:else}
          <p class="px-4 py-10 text-center text-sm text-muted-foreground">
            No profiles yet. Log into League, then save your current settings from the tray.
          </p>
        {/each}
      </section>

      {#if message}
        <p class="text-sm" class:text-destructive={message.failed} class:text-muted-foreground={!message.failed}>
          {message.text}
        </p>
      {/if}

      <header class="pt-2">
        <h2 class="text-lg font-semibold tracking-tight">Champions</h2>
        <p class="text-sm text-muted-foreground">
          Settings a champion uses instead of your profile's. They go on when you pick the champion and come off
          after the game.
        </p>
      </header>

      <section class="overflow-hidden rounded-lg border border-border">
        {#each view?.overlays ?? [] as overlay, index (overlay.champion.id)}
          {@const id = `champion-${overlay.champion.id}`}
          <div class="group flex min-h-14 items-center gap-3 px-4 py-2.5" class:border-t={index > 0}>
            <img src={overlay.champion.icon} alt="" class="size-8 shrink-0 rounded-md" />
            {#if editing?.id === id}
              <p class="min-w-0 flex-1 truncate text-sm">
                Delete <span class="font-medium">{overlay.champion.name}</span>'s settings?
              </p>
              <Button
                variant="destructive"
                size="sm"
                disabled={busy !== null}
                onclick={() => run(id, () => api.deleteOverlay(overlay.champion.id))}
              >
                Delete
              </Button>
              <Button variant="ghost" size="sm" onclick={() => (editing = null)}>Cancel</Button>
            {:else}
              <div class="min-w-0 flex-1">
                <p class="flex items-center gap-2 truncate text-sm font-medium">
                  {overlay.champion.name}
                  {#if view?.activeOverlay?.id === overlay.champion.id}<Badge variant="secondary">on now</Badge>{/if}
                </p>
                <p class="truncate text-xs text-muted-foreground" title={overlay.settings.map(([key, value]) => `${key} = ${value}`).join("\n")}>
                  {overlay.settings.map(([key, value]) => `${key.replace(/^evn?t/, "")} ${value || "none"}`).join(" · ")}
                </p>
              </div>
              <Button
                class="opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
                variant="ghost"
                size="icon"
                onclick={() => (editing = { id, mode: "delete", name: overlay.champion.name })}
                aria-label="Delete"
                title="Delete"
              >
                <Trash2 />
              </Button>
            {/if}
          </div>
        {:else}
          <p class="px-4 py-10 text-center text-sm text-muted-foreground">
            None yet. Play a champion, change settings in the game, and choose
            <span class="text-foreground">Only for that champion</span> when mimic asks afterwards.
          </p>
        {/each}
      </section>
    </main>
  </ScrollArea>
</div>
