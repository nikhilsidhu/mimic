<!-- App-wide switches. -->
<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import X from "@lucide/svelte/icons/x";
  import { onMount } from "svelte";
  import SectionHeader from "$lib/components/section-header.svelte";
  import SettingRow from "$lib/components/setting-row.svelte";
  import { Button } from "$lib/components/ui/button";
  import { Switch } from "$lib/components/ui/switch";
  import { attempt, type Report } from "$lib/attempt";
  import { settingLabel } from "$lib/binds";
  import * as api from "$lib/api";
  import { getTheme, setTheme, THEMES, type Theme } from "$lib/theme";

  let { onmessage }: { onmessage: Report } = $props();

  let autostart = $state(false);
  let notices = $state(true);
  let theme = $state<Theme>("black");
  let update = $state<api.UpdateStatus | null>(null);
  let updating = $state(false);
  // A check just now found nothing newer.
  let upToDate = $state(false);
  let install = $state<string | null>(null);

  // Settings muted from the prompt about changed settings, as `file/section/key`.
  let muted = $state<string[]>([]);

  const refreshInstall = async () => (install = await api.getInstallPath());
  const refreshMuted = async () => (muted = await api.getMutedSettings());
  const refreshUpdate = async () => (update = await api.getUpdateStatus());

  onMount(() => {
    theme = getTheme();
    api.getAutostart().then((enabled) => (autostart = enabled));
    api.getShowsNotices().then((enabled) => (notices = enabled));
    refreshInstall();
    refreshMuted();
    refreshUpdate();
    // mimic picks a chosen folder up within seconds; the view changes when it does.
    return api.onViewChanged(() => {
      refreshInstall();
      refreshMuted();
      refreshUpdate();
    });
  });

  async function chooseInstall() {
    await attempt(onmessage, api.chooseInstall);
    refreshInstall();
  }

  // Installs the newer version if one is known, and looks for one otherwise.
  async function updateOrCheck() {
    if (updating) return;
    updating = true;
    const worked = await attempt(onmessage, async () => {
      if (update?.available) return api.installUpdate();
      await api.checkUpdate();
    });
    await refreshUpdate();
    upToDate = worked && !update?.available;
    updating = false;
  }

  // A switch shows what is stored, so it is read back whether or not the change worked.
  async function setNotices(enabled: boolean) {
    await attempt(onmessage, () => api.setShowsNotices(enabled));
    notices = await api.getShowsNotices();
  }

  async function setAutostart(enabled: boolean) {
    await attempt(onmessage, () => api.setAutostart(enabled));
    autostart = await api.getAutostart();
  }

  async function unmute(id: string) {
    await attempt(onmessage, () => api.unmuteSetting(id));
    refreshMuted();
  }
</script>

<SectionHeader title="Settings" />

<section class="list-box">
  <SettingRow title="Start with Windows" label>
    {#snippet description()}
      <p class="text-xs text-muted-foreground">Starts in the tray, without opening a window.</p>
    {/snippet}
    <Switch checked={autostart} onCheckedChange={setAutostart} />
  </SettingRow>

  <SettingRow title="Notices" label>
    {#snippet description()}
      <p class="text-xs text-muted-foreground">Pop up when settings are applied automatically.</p>
    {/snippet}
    <Switch checked={notices} onCheckedChange={setNotices} />
  </SettingRow>

  <SettingRow title="Theme">
    <div class="flex rounded-md border border-border p-0.5" role="radiogroup" aria-label="Theme">
      {#each THEMES as option (option)}
        <button
          class="rounded px-2.5 py-1 text-xs capitalize text-muted-foreground aria-checked:bg-live aria-checked:font-medium aria-checked:text-background"
          role="radio"
          aria-checked={theme === option}
          onclick={() => {
            theme = option;
            setTheme(option);
          }}
        >
          {option}
        </button>
      {/each}
    </div>
  </SettingRow>

  <SettingRow title="League folder">
    {#snippet description()}
      <p class="truncate text-xs" class:text-muted-foreground={install} class:text-destructive={!install} title={install}>
        {install ?? "Not found. Choose the folder that holds LeagueClient.exe."}
      </p>
    {/snippet}
    <Button variant="outline" size="sm" onclick={chooseInstall}>Choose…</Button>
  </SettingRow>

  <SettingRow title="Data">
    {#snippet description()}
      <p class="text-xs text-muted-foreground">Profiles, history and logs.</p>
    {/snippet}
    <Button variant="outline" size="sm" onclick={() => api.openDataFolder()}>Open folder</Button>
  </SettingRow>

  <SettingRow title="Version">
    <span class="text-xs text-faint">{update?.current ?? ""}</span>
    {#if update?.available}
      <Button size="sm" disabled={updating} onclick={updateOrCheck}>
        {updating ? "Updating…" : `Update to ${update.available}`}
      </Button>
    {:else}
      <Button
        variant="ghost"
        size="icon"
        disabled={updating}
        onclick={updateOrCheck}
        aria-label={upToDate ? "Up to date. Check again" : "Check for updates"}
        title={upToDate ? "Up to date" : "Check for updates"}
      >
        {#if upToDate}
          <Check class="text-live" />
        {:else}
          <RefreshCw class={updating ? "animate-spin" : ""} />
        {/if}
      </Button>
    {/if}
  </SettingRow>

  {#if muted.length}
    <div class="border-t border-border px-4 py-2.5">
      <p class="text-sm font-medium">Muted settings</p>
      <p class="text-xs text-muted-foreground">No prompt when these change.</p>
      <ul class="mt-2 flex flex-wrap gap-1.5">
        {#each muted as id (id)}
          <li class="flex items-center gap-1 rounded-md border border-border py-0.5 pr-1 pl-2 text-xs" title={id}>
            {settingLabel(id.split("/").at(-1) ?? id)}
            <button
              class="icon-glow rounded p-0.5 text-faint hover:text-hover"
              aria-label="Ask about {id} again"
              title="Ask about this again"
              onclick={() => unmute(id)}
            >
              <X class="size-3" />
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</section>
