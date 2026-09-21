<!-- App-wide switches. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { Switch } from "$lib/components/ui/switch";
  import * as api from "$lib/api";

  let { onmessage }: { onmessage: (text: string, failed: boolean) => void } = $props();

  let autostart = $state(false);

  onMount(async () => (autostart = await api.getAutostart()));

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
</section>
