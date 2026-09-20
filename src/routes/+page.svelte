<script lang="ts">
  import { onMount, tick } from "svelte";
  import Check from "@lucide/svelte/icons/check";
  import Download from "@lucide/svelte/icons/download";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Upload from "@lucide/svelte/icons/upload";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import AccountsSection from "$lib/components/accounts-section.svelte";
  import AddChampion from "$lib/components/add-champion.svelte";
  import Avatar from "$lib/components/avatar.svelte";
  import Bind from "$lib/components/bind.svelte";
  import SnapshotsSection from "$lib/components/snapshots-section.svelte";
  import Titlebar from "$lib/components/titlebar.svelte";
  import * as api from "$lib/api";
  import { settingLabel } from "$lib/binds";

  let view = $state<api.View | null>(null);
  /** The profile an action is running on. One action at a time. */
  let busy = $state<string | null>(null);
  let message = $state<{ text: string; failed: boolean } | null>(null);
  /** The row being renamed or asking to confirm a delete, if any. */
  let editing = $state<{ id: string; mode: "rename" | "delete"; name: string } | null>(null);
  let renameInput = $state<HTMLInputElement | null>(null);
  /** Whether the champion picker is open. */
  let adding = $state(false);

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

  /** Export and import open a file dialog; cancelling it is not worth a message. */
  async function transfer(action: () => Promise<string | null>) {
    try {
      const said = await action();
      if (said) message = { text: said, failed: false };
    } catch (err) {
      message = { text: String(err), failed: true };
    }
    refresh();
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
      <header class="flex items-center gap-4">
        <Avatar account={view?.account} size="lg" />
        <div class="min-w-0 flex-1">
          <h1 class="text-lg font-semibold tracking-tight">Profiles</h1>
          <p class="truncate text-sm text-muted-foreground">{view?.account?.name ?? view?.status ?? "Starting…"}</p>
        </div>
        {#if view?.activity}
          <Badge variant="secondary">{view.activity}</Badge>
        {:else if view && !view.connected}
          <Badge variant="outline">{view.status}</Badge>
        {/if}
        <Button variant="outline" size="sm" onclick={() => transfer(api.importProfile)}><Upload />Import</Button>
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
                Delete <span class="font-medium">{profile.name}</span>?
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
                <p class="text-xs text-faint">
                  {view?.pending === profile.name ? "Applies at next login" : `${profile.settings} settings`}
                </p>
              </div>
              <div class="flex items-center gap-1 text-faint transition-colors group-hover:text-muted-foreground">
                <Button variant="ghost" size="icon" onclick={() => startRename(profile)} aria-label="Rename" title="Rename">
                  <Pencil />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onclick={() => transfer(() => api.exportProfile(profile.id))}
                  aria-label="Export"
                  title="Export to a file"
                >
                  <Download />
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
            No profiles yet. Save one from the tray panel.
          </p>
        {/each}
      </section>

      {#if message}
        <p class="text-sm" class:text-destructive={message.failed} class:text-muted-foreground={!message.failed}>
          {message.text}
        </p>
      {/if}

      <header class="flex items-end justify-between gap-4 pt-2">
        <div>
          <h2 class="text-lg font-semibold tracking-tight">Champions</h2>
          <p class="text-sm text-muted-foreground">
            Overrides used only while you play that champion.
          </p>
        </div>
        {#if !adding}
          <Button variant="outline" size="sm" onclick={() => (adding = true)}><Plus />Add champion</Button>
        {/if}
      </header>

      {#if adding}
        <AddChampion
          oncancel={() => (adding = false)}
          onsaved={(said) => {
            adding = false;
            message = { text: said, failed: false };
            refresh();
          }}
        />
      {/if}

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
                <p class="flex flex-wrap items-center gap-x-3 gap-y-1 pt-0.5 text-xs text-muted-foreground">
                  {#each overlay.settings as [key, value] (key)}
                    <span class="inline-flex items-center gap-1.5">{settingLabel(key)} <Bind {value} /></span>
                  {/each}
                </p>
              </div>
              <Button
                class="text-faint transition-colors group-hover:text-muted-foreground"
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
            None yet.
          </p>
        {/each}
      </section>

      <AccountsSection onmessage={(text, failed) => (message = { text, failed })} />
      <SnapshotsSection onmessage={(text, failed) => (message = { text, failed })} />
    </main>
  </ScrollArea>
</div>
