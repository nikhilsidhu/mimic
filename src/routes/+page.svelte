<script lang="ts">
  import { onMount, tick } from "svelte";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import CopyPlus from "@lucide/svelte/icons/copy-plus";
  import Download from "@lucide/svelte/icons/download";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Plus from "@lucide/svelte/icons/plus";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Upload from "@lucide/svelte/icons/upload";
  import X from "@lucide/svelte/icons/x";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import AccountsSection from "$lib/components/accounts-section.svelte";
  import AddChampion from "$lib/components/add-champion.svelte";
  import Avatar from "$lib/components/avatar.svelte";
  import Bind from "$lib/components/bind.svelte";
  import ProfileDetails from "$lib/components/profile-details.svelte";
  import SettingsSection from "$lib/components/settings-section.svelte";
  import SnapshotsSection from "$lib/components/snapshots-section.svelte";
  import Status from "$lib/components/status.svelte";
  import Titlebar from "$lib/components/titlebar.svelte";
  import Toast from "$lib/components/toast.svelte";
  import * as api from "$lib/api";
  import { isBind, settingLabel } from "$lib/binds";
  import { DOT_LIVE, DOT_OFF, WAITING } from "$lib/tones";

  let view = $state<api.View | null>(null);
  /** The profile an action is running on. One action at a time. */
  let busy = $state<string | null>(null);
  let message = $state<{ text: string; failed: boolean } | null>(null);
  /** The row being renamed or asking to confirm a delete, if any. */
  let editing = $state<{ id: string; mode: "rename" | "delete" | "update"; name: string } | null>(null);
  // The champion override whose removal is being confirmed, as `champion-id/setting`.
  let removing = $state<string | null>(null);

  // A card shows this many overrides until it is asked for the rest, so that one champion
  // with many does not push the others off the screen.
  const CARD_ROWS = 4;
  let unfolded = $state<number[]>([]);
  // The champion whose card shows its remove and delete buttons.
  let editingCard = $state<number | null>(null);
  // Whether the header shows every account, not just the one in use.
  let showAccounts = $state(false);
  // With this many champions a filter appears.
  const FILTER_FROM = 9;
  let championFilter = $state("");
  const champions = $derived(
    (view?.overlays ?? []).filter((overlay) =>
      overlay.champion.name.toLowerCase().includes(championFilter.trim().toLowerCase()),
    ),
  );
  /** The profile whose contents are shown, by id. */
  let opened = $state<string | null>(null);
  let renameInput = $state<HTMLInputElement | null>(null);
  /** Whether the champion picker is open. */
  let adding = $state(false);

  const refresh = async () => (view = await api.getView());

  onMount(() => {
    refresh();
    const stopView = api.onViewChanged(refresh);
    // The tray panel sends people here to save settings for a champion.
    const stopAdd = api.onAddChampion(async () => {
      adding = true;
      await tick();
      document.getElementById("champions")?.scrollIntoView({ behavior: "smooth", block: "start" });
    });
    return () => {
      stopView();
      stopAdd();
    };
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

      <header class="flex items-end justify-between gap-4 pt-2">
        <div>
          <h2 class="text-lg font-semibold tracking-tight">Profiles</h2>
          <p class="text-sm text-muted-foreground">Saved settings you can apply to any account.</p>
        </div>
        <Button variant="outline" size="sm" onclick={() => transfer(api.importProfile)}><Upload />Import</Button>
      </header>

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
                  <RefreshCw />
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

      <header id="champions" class="flex scroll-mt-4 items-end justify-between gap-4 pt-2">
        <div>
          <h2 class="text-lg font-semibold tracking-tight">Champions</h2>
          <p class="text-sm text-muted-foreground">
            Overrides used only while you play that champion.
          </p>
        </div>
        <div class="flex items-center gap-2">
          {#if (view?.overlays.length ?? 0) >= FILTER_FROM}
            <Input class="h-6 w-36" bind:value={championFilter} placeholder="Filter…" />
          {/if}
          {#if !adding}
            <Button variant="outline" size="sm" onclick={() => (adding = true)}><Plus />Add champion</Button>
          {/if}
        </div>
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

      <!-- A card per champion, two to a row: narrow lists keep each value next to its name
           and are read top to bottom. -->
      <section class="grid gap-3 min-[720px]:grid-cols-2">
        {#each champions as overlay (overlay.champion.id)}
          {@const id = `champion-${overlay.champion.id}`}
          {@const folded = overlay.settings.length > CARD_ROWS && !unfolded.includes(overlay.champion.id)}
          <div class="group rounded-lg border border-border px-3 pt-2.5 pb-2">
            <div class="flex h-8 items-center gap-2.5">
              <img src={overlay.champion.icon} alt="" class="size-7 shrink-0 rounded-md" />
              {#if editing?.id === id}
                <p class="min-w-0 flex-1 truncate text-sm">Delete {overlay.champion.name}?</p>
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
                <p class="min-w-0 truncate text-sm font-medium">{overlay.champion.name}</p>
                {#if view?.activeOverlay?.id === overlay.champion.id}<Status>active</Status>{/if}
                <span class="flex-1"></span>
                <!-- A card is read far more often than it is changed, so what changes it shows
                     only once asked for. Nothing depends on hovering. -->
                {#if editingCard === overlay.champion.id}
                  <Button
                    class="text-muted-foreground"
                    variant="ghost"
                    size="icon-sm"
                    onclick={() => (editing = { id, mode: "delete", name: overlay.champion.name })}
                    aria-label="Delete {overlay.champion.name}'s settings"
                    title="Delete all of {overlay.champion.name}'s settings"
                  >
                    <Trash2 />
                  </Button>
                  <Button class="-mr-1" variant="secondary" size="sm" onclick={() => (editingCard = null)}>Done</Button>
                {:else}
                  <Button
                    class="-mr-1 text-faint hover:text-foreground"
                    variant="ghost"
                    size="icon-sm"
                    onclick={() => (editingCard = overlay.champion.id)}
                    aria-label="Edit {overlay.champion.name}'s settings"
                    title="Edit"
                  >
                    <Pencil />
                  </Button>
                {/if}
              {/if}
            </div>
            <ul class="pt-1 text-xs">
              {#each folded ? overlay.settings.slice(0, CARD_ROWS) : overlay.settings as setting (api.muteId(setting))}
                <!-- Remove, and the question it leads to, lie over the right end of the row, so
                     that they take no room of their own and nothing moves when they appear. -->
                <li class="relative grid min-h-8 items-center gap-2 {editingCard === overlay.champion.id ? 'grid-cols-[minmax(0,1fr)_auto_auto]' : 'grid-cols-[minmax(0,1fr)_auto]'}">
                  <span class="leading-tight text-muted-foreground" title="{setting.section} / {setting.key}">
                    {settingLabel(setting.key)}
                  </span>
                  <!-- What the champion changes it from, when that is known, and to. -->
                  <span class="flex items-center justify-end gap-1.5">
                    {#if setting.from !== null}
                      {#if isBind(setting)}<Bind value={setting.from} />{:else}<span>{setting.from}</span>{/if}
                      <ArrowRight class="size-3 shrink-0 text-faint" />
                    {/if}
                    {#if isBind(setting)}<Bind value={setting.value} />{:else}<span class="font-medium">{setting.value}</span>{/if}
                  </span>
                  {#if removing === `${id}/${api.muteId(setting)}`}
                    <span class="absolute inset-y-0 right-0 flex items-center gap-1 bg-linear-to-l from-background from-85% to-transparent pl-8">
                      <Button
                        variant="destructive"
                        size="sm"
                        disabled={busy !== null}
                        onclick={() => {
                          removing = null;
                          run(id, () => api.removeOverlaySetting(overlay.champion.id, setting));
                        }}
                      >
                        Remove
                      </Button>
                      <Button variant="ghost" size="sm" onclick={() => (removing = null)}>Cancel</Button>
                    </span>
                  {:else if editingCard === overlay.champion.id}
                    <span class="flex items-center">
                      <button
                        class="-mr-1 flex size-6 items-center justify-center rounded text-muted-foreground hover:text-foreground"
                        disabled={busy !== null}
                        title="Remove this setting from {overlay.champion.name}"
                        aria-label="Remove {settingLabel(setting.key)} from {overlay.champion.name}"
                        onclick={() => (removing = `${id}/${api.muteId(setting)}`)}
                      >
                        <X class="size-3.5" />
                      </button>
                    </span>
                  {/if}
                </li>
              {/each}
            </ul>
            {#if overlay.settings.length > CARD_ROWS}
              <button
                class="pt-1 text-xs text-faint hover:text-foreground"
                onclick={() =>
                  (unfolded = folded
                    ? [...unfolded, overlay.champion.id]
                    : unfolded.filter((champion) => champion !== overlay.champion.id))}
              >
                {folded ? `Show ${overlay.settings.length - CARD_ROWS} more` : "Show fewer"}
              </button>
            {/if}
          </div>
        {:else}
          <p class="col-span-full rounded-lg border border-border px-4 py-10 text-center text-sm text-muted-foreground">
            {view?.overlays.length ? "No champion by that name." : "None yet."}
          </p>
        {/each}
      </section>
      <SettingsSection onmessage={(text, failed) => (message = { text, failed })} />
      <SnapshotsSection onmessage={(text, failed) => (message = { text, failed })} />
    </main>
  </ScrollArea>
  <Toast bind:message />
</div>
