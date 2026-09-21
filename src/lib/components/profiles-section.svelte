<!-- The saved profiles: apply one, look inside it, rename, overwrite, duplicate, export, delete. -->
<script lang="ts">
  import { tick } from "svelte";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import CopyPlus from "@lucide/svelte/icons/copy-plus";
  import Download from "@lucide/svelte/icons/download";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Save from "@lucide/svelte/icons/save";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Upload from "@lucide/svelte/icons/upload";
  import Confirm from "$lib/components/confirm.svelte";
  import ProfileDetails from "$lib/components/profile-details.svelte";
  import SectionHeader from "$lib/components/section-header.svelte";
  import Status from "$lib/components/status.svelte";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import * as api from "$lib/api";
  import { attempt, type Report } from "$lib/attempt";
  import { DOT_WAITING, WAITING } from "$lib/tones";

  type Props = { view: api.View | null; onmessage: Report; onchanged: () => void };
  let { view, onmessage, onchanged }: Props = $props();

  /** The profile an action is running on. One action at a time. */
  let busy = $state<string | null>(null);
  /** The row being renamed, or asking before it is deleted or overwritten. */
  let editing = $state<{ id: string; mode: "rename" | "delete" | "update"; name: string } | null>(null);
  /** The profile whose contents are shown, by id. */
  let opened = $state<string | null>(null);
  let renameInput = $state<HTMLInputElement | null>(null);

  // Development only: `VITE_OPEN_PROFILE=1` starts with the first profile opened, for looking
  // at that view and taking screenshots of it without a click.
  let openedForDemo = false;
  $effect(() => {
    const first = view?.profiles[0];
    if (import.meta.env.VITE_OPEN_PROFILE && first && !openedForDemo) {
      openedForDemo = true;
      opened = first.id;
    }
  });

  async function run(id: string, action: () => Promise<string>) {
    if (busy) return;
    busy = id;
    if (await attempt(onmessage, action)) editing = null;
    busy = null;
    onchanged();
  }

  /** Export and import open a file dialog; cancelling it says nothing. */
  async function transfer(action: () => Promise<string | null>) {
    await attempt(onmessage, action);
    onchanged();
  }

  async function startRename(profile: api.ProfileView) {
    editing = { id: profile.id, mode: "rename", name: profile.name };
    await tick();
    renameInput?.select();
  }

  function submitRename(event: SubmitEvent) {
    event.preventDefault();
    const { id, name } = editing ?? {};
    if (id && name) run(id, () => api.renameProfile(id, name));
  }
</script>

<SectionHeader title="Profiles" description="Saved settings you can apply to any account.">
  <Button variant="outline" size="sm" onclick={() => transfer(api.importProfile)}><Upload />Import</Button>
</SectionHeader>

<section class="list-box">
  {#each view?.profiles ?? [] as profile, index (profile.id)}
    <div class="group list-row" class:border-t={index > 0}>
      <button
        class="flex shrink-0 items-center gap-1 pressable text-faint"
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
        <p class="min-w-0 flex-1 truncate text-sm">Delete <span class="font-medium">{profile.name}</span>?</p>
        <Confirm
          action="Delete"
          disabled={busy !== null}
          onconfirm={() => run(profile.id, () => api.deleteProfile(profile.id))}
          oncancel={() => (editing = null)}
        />
      {:else if editing?.id === profile.id && editing.mode === "update"}
        <p class="min-w-0 flex-1 truncate text-sm">
          Overwrite <span class="font-medium">{profile.name}</span> with this account's settings?
        </p>
        <Confirm
          action="Overwrite"
          destructive={false}
          disabled={busy !== null}
          onconfirm={() => run(profile.id, () => api.updateProfile(profile.id))}
          oncancel={() => (editing = null)}
        />
      {:else}
        <!-- The name opens the profile too; the arrow beside it is the same control for the keyboard. -->
        <button
          class="min-w-0 flex-1 text-left"
          tabindex="-1"
          aria-hidden="true"
          onclick={() => (opened = opened === profile.id ? null : profile.id)}
        >
          <span class="flex h-5 items-center gap-2 truncate text-sm font-medium">
            {profile.name}
            {#if profile.active}<Status>active</Status>{/if}
          </span>
          {#if view?.pending === profile.name}
            <span class="mt-0.5 block text-xs {WAITING}">Applies at next login</span>
          {:else}
            <!-- A state on the profile in use, as a document has unsaved changes: the others differ
                 from the account by design. Either the account was changed since the profile was
                 applied, or the profile was, from another account. -->
            <span class="mt-0.5 flex items-center gap-1.5 text-xs text-faint">
              {profile.settings} settings
              {#if profile.active && (view?.changed || profile.differs)}
                <span
                  class="flex items-center gap-1.5 text-muted-foreground"
                  title={view?.changed
                    ? `Settings changed on this account since ${profile.name} was applied. Save them, or review them from the tray.`
                    : `${profile.name} has changed since it was applied to this account. Apply it to catch up.`}
                >
                  · <span class="size-1.5 rounded-full {DOT_WAITING}"></span>{view?.changed ? "unsaved changes" : "not applied"}
                </span>
              {/if}
            </span>
          {/if}
        </button>
        <!-- Always there, so that nobody has to find them by hovering, but faint until the row is
             hovered or tabbed into. -->
        <div
          class="flex items-center gap-1 text-muted-foreground opacity-35 transition-opacity duration-200 group-hover:opacity-100 focus-within:opacity-100"
        >
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
            class="hover-danger"
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
          title={profile.differs ? `Changes ${profile.differs} settings on this account` : profile.differs === 0 ? "Already matches this account" : undefined}
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
    <p class="px-4 py-10 text-center text-sm text-muted-foreground">No profiles yet. Save one from the tray panel.</p>
  {/each}
</section>
