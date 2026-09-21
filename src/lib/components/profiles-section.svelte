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
  import { WAITING } from "$lib/tones";

  type Props = { view: api.View | null; onmessage: Report; onchanged: () => void };
  let { view, onmessage, onchanged }: Props = $props();

  /** The profile an action is running on. One action at a time. */
  let busy = $state<string | null>(null);
  /** The row being renamed, or asking before it is deleted or overwritten. */
  let editing = $state<{ id: string; mode: "rename" | "delete" | "update"; name: string } | null>(null);
  /** The profile whose contents are shown, by id. */
  let opened = $state<string | null>(null);
  let renameInput = $state<HTMLInputElement | null>(null);

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
        <Button variant="outline" size="sm" disabled={busy !== null} onclick={() => run(profile.id, () => api.applyProfile(profile.id))}>
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
