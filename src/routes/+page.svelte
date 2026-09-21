<script lang="ts">
  import { onMount, tick } from "svelte";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import CopyPlus from "@lucide/svelte/icons/copy-plus";
  import Download from "@lucide/svelte/icons/download";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Save from "@lucide/svelte/icons/save";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Upload from "@lucide/svelte/icons/upload";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import AccountsSection from "$lib/components/accounts-section.svelte";
  import ChampionsSection from "$lib/components/champions-section.svelte";
  import SectionHeader from "$lib/components/section-header.svelte";
  import Avatar from "$lib/components/avatar.svelte";
  import ProfileDetails from "$lib/components/profile-details.svelte";
  import SettingsSection from "$lib/components/settings-section.svelte";
  import SnapshotsSection from "$lib/components/snapshots-section.svelte";
  import Status from "$lib/components/status.svelte";
  import Titlebar from "$lib/components/titlebar.svelte";
  import Toast from "$lib/components/toast.svelte";
  import * as api from "$lib/api";
  import { DOT_LIVE, DOT_OFF, WAITING } from "$lib/tones";

  let view = $state<api.View | null>(null);
  /** The profile an action is running on. One action at a time. */
  let busy = $state<string | null>(null);
  let message = $state<{ text: string; failed: boolean } | null>(null);
  /** The row being renamed or asking to confirm a delete, if any. */
  let editing = $state<{ id: string; mode: "rename" | "delete" | "update"; name: string } | null>(null);
  /** The profile whose contents are shown, by id. */
  let opened = $state<string | null>(null);
  let renameInput = $state<HTMLInputElement | null>(null);
  /** Whether the champion picker is open. */
  // Whether the header shows every account, not just the one in use.
  let showAccounts = $state(false);

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
    <main class="mx-auto flex max-w-4xl flex-col gap-6 px-6 pt-6 pb-10">
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
        <!-- The other accounts fold out of the one in use. -->
        <Button variant="ghost" size="sm" aria-expanded={showAccounts} onclick={() => (showAccounts = !showAccounts)}>
          Accounts
          <ChevronRight class="transition-transform {showAccounts ? 'rotate-90' : ''}" />
        </Button>
      </header>
      {#if showAccounts}
        <AccountsSection onmessage={(text, failed) => (message = { text, failed })} />
      {/if}

      <SectionHeader title="Profiles" description="Saved settings you can apply to any account.">
        <Button variant="outline" size="sm" onclick={() => transfer(api.importProfile)}><Upload />Import</Button>
      </SectionHeader>

      <section class="overflow-hidden rounded-lg border border-border">
        {#each view?.profiles ?? [] as profile, index (profile.id)}
          <div class="group flex min-h-14 items-center gap-3 px-4 py-2.5" class:border-t={index > 0}>
            <button
              class="flex shrink-0 items-center gap-1 text-faint hover:text-foreground"
              aria-label="Show settings"
              aria-expanded={opened === profile.id}
              onclick={() => (opened = opened === profile.id ? null : profile.id)}
            >
              <ChevronRight class="size-3.5 transition-transform {opened === profile.id ? 'rotate-90' : ''}" />
            </button>

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
            {:else if editing?.id === profile.id && editing.mode === "update"}
              <p class="min-w-0 flex-1 truncate text-sm">
                Overwrite <span class="font-medium">{profile.name}</span> with this account's settings?
              </p>
              <Button size="sm" disabled={busy !== null} onclick={() => run(profile.id, () => api.updateProfile(profile.id))}>
                Overwrite
              </Button>
              <Button variant="ghost" size="sm" onclick={() => (editing = null)}>Cancel</Button>
            {:else}
              <div class="min-w-0 flex-1">
                <p class="flex items-center gap-2 truncate text-sm font-medium">
                  {profile.name}
                  {#if profile.active}<Status>active</Status>{/if}
                </p>
                {#if view?.pending === profile.name}
                  <p class="text-xs {WAITING}">Applies at next login</p>
                {:else}
                  <p class="text-xs text-faint">{profile.settings} settings</p>
                {/if}
              </div>
              <div class="flex items-center gap-1 text-faint transition-colors group-hover:text-muted-foreground">
                <Button variant="ghost" size="icon" onclick={() => startRename(profile)} aria-label="Rename" title="Rename">
                  <Pencil />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  disabled={!view?.connected}
                  onclick={() => (editing = { id: profile.id, mode: "update", name: profile.name })}
                  aria-label="Update from this account"
                  title={view?.connected ? "Overwrite with this account's current settings" : "Log into League to update a profile"}
                >
                  <Save />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onclick={() => run(profile.id, () => api.duplicateProfile(profile.id))}
                  aria-label="Duplicate"
                  title="Duplicate"
                >
                  <CopyPlus />
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
          {#if opened === profile.id}
            <ProfileDetails id={profile.id} />
          {/if}
        {:else}
          <p class="px-4 py-10 text-center text-sm text-muted-foreground">
            No profiles yet. Save one from the tray panel.
          </p>
        {/each}
      </section>

      <ChampionsSection {view} onmessage={(text, failed) => (message = { text, failed })} onchanged={refresh} />
      <SettingsSection onmessage={(text, failed) => (message = { text, failed })} />
      <SnapshotsSection onmessage={(text, failed) => (message = { text, failed })} />
    </main>
  </ScrollArea>
  <Toast bind:message />
</div>
