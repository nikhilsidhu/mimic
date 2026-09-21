<!-- App-wide switches. -->
<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import X from "@lucide/svelte/icons/x";
  import { onMount } from "svelte";
  import { Button } from "$lib/components/ui/button";
  import { Switch } from "$lib/components/ui/switch";
  import { settingLabel } from "$lib/binds";
  import * as api from "$lib/api";
  import { getTheme, setTheme, type Theme } from "$lib/theme";

  let { onmessage }: { onmessage: (text: string, failed: boolean) => void } = $props();

  let autostart = $state(false);
  let notices = $state(true);
  let theme = $state<Theme>("system");
  let update = $state<api.UpdateStatus | null>(null);
  let updating = $state(false);
  // A check just now found nothing newer.
  let upToDate = $state(false);
  const themes: Theme[] = ["system", "light", "dark"];
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
    try {
      const said = await api.chooseInstall();
      if (said) onmessage(said, false);
    } catch (err) {
      onmessage(String(err), true);
    }
    refreshInstall();
  }

  // Installs the newer version if one is known, and looks for one otherwise.
  async function updateOrCheck() {
    if (updating) return;
    updating = true;
    try {
      if (update?.available) await api.installUpdate();
      else {
        await api.checkUpdate();
        await refreshUpdate();
        upToDate = !update?.available;
      }
    } catch (err) {
      onmessage(String(err), true);
    } finally {
      updating = false;
    }
    refreshUpdate();
  }

  async function setNotices(enabled: boolean) {
    try {
      await api.setShowsNotices(enabled);
    } catch (err) {
      onmessage(String(err), true);
    }
    notices = await api.getShowsNotices();
  }

  async function unmute(id: string) {
    try {
      await api.unmuteSetting(id);
    } catch (err) {
      onmessage(String(err), true);
    }
    refreshMuted();
  }

  async function setAutostart(enabled: boolean) {
    try {
      await api.setAutostart(enabled);
    } catch (err) {
      onmessage(String(err), true);
    }
    autostart = await api.getAutostart();
  }
</script>

<header class="pt-2">
  <h2 class="text-lg font-semibold tracking-tight">Settings</h2>
</header>

<section class="overflow-hidden rounded-lg border border-border">
  <label class="flex min-h-14 items-center gap-3 px-4 py-2.5">
    <div class="min-w-0 flex-1">
      <p class="text-sm font-medium">Start with Windows</p>
      <p class="text-xs text-muted-foreground">Starts in the tray, without opening a window.</p>
    </div>
    <Switch checked={autostart} onCheckedChange={setAutostart} />
  </label>
  <label class="flex min-h-14 items-center gap-3 border-t border-border px-4 py-2.5">
    <div class="min-w-0 flex-1">
      <p class="text-sm font-medium">Notices</p>
      <p class="text-xs text-muted-foreground">Pop up when settings are applied automatically.</p>
    </div>
    <Switch checked={notices} onCheckedChange={setNotices} />
  </label>
  <div class="flex min-h-14 items-center gap-3 border-t border-border px-4 py-2.5">
    <div class="min-w-0 flex-1">
      <p class="text-sm font-medium">Theme</p>
    </div>
    <div class="flex rounded-md border border-border p-0.5" role="radiogroup" aria-label="Theme">
      {#each themes as option (option)}
        <button
          class="rounded px-2.5 py-1 text-xs capitalize text-muted-foreground aria-checked:bg-accent aria-checked:text-accent-foreground"
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
  </div>
  <div class="flex min-h-14 items-center gap-3 border-t border-border px-4 py-2.5">
    <div class="min-w-0 flex-1">
      <p class="text-sm font-medium">League folder</p>
      <p class="truncate text-xs" class:text-muted-foreground={install} class:text-destructive={!install} title={install}>
        {install ?? "Not found. Choose the folder that holds LeagueClient.exe."}
      </p>
    </div>
    <Button variant="outline" size="sm" onclick={chooseInstall}>Choose…</Button>
  </div>
  <div class="flex min-h-14 items-center gap-3 border-t border-border px-4 py-2.5">
    <div class="min-w-0 flex-1">
      <p class="text-sm font-medium">Data</p>
      <p class="text-xs text-muted-foreground">Profiles, history and logs.</p>
    </div>
    <Button variant="outline" size="sm" onclick={() => api.openDataFolder()}>Open folder</Button>
  </div>
  <div class="flex min-h-14 items-center gap-3 border-t border-border px-4 py-2.5">
    <p class="min-w-0 flex-1 text-sm font-medium">Version</p>
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
          <Check />
        {:else}
          <RefreshCw class={updating ? "animate-spin" : ""} />
        {/if}
      </Button>
    {/if}
  </div>
  {#if muted.length}
    <div class="border-t border-border px-4 py-2.5">
      <p class="text-sm font-medium">Muted settings</p>
      <p class="text-xs text-muted-foreground">Changes to these are kept without asking.</p>
      <ul class="mt-2 flex flex-wrap gap-1.5">
        {#each muted as id (id)}
          <li class="flex items-center gap-1 rounded-md border border-border py-0.5 pr-1 pl-2 text-xs" title={id}>
            {settingLabel(id.split("/").at(-1) ?? id)}
            <button
              class="rounded p-0.5 text-faint hover:text-foreground"
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
