<!-- Every account mimic has seen: which profile it is on, and whether it auto-applies. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import { Switch } from "$lib/components/ui/switch";
  import * as api from "$lib/api";

  let { onmessage }: { onmessage: (text: string, failed: boolean) => void } = $props();

  let accounts = $state<api.AccountRow[]>([]);
  /** The account asking to confirm being forgotten, by puuid. */
  let forgetting = $state<string | null>(null);

  const refresh = async () => (accounts = await api.getAccounts());

  onMount(() => {
    refresh();
    return api.onViewChanged(refresh);
  });

  async function setAutoApply(account: api.AccountRow, enabled: boolean) {
    try {
      await api.setAccountAutoApply(account.puuid, enabled);
    } catch (err) {
      onmessage(String(err), true);
    }
    refresh();
  }

  async function forget(account: api.AccountRow) {
    try {
      onmessage(await api.forgetAccount(account.puuid), false);
    } catch (err) {
      onmessage(String(err), true);
    }
    forgetting = null;
    refresh();
  }
</script>

<header class="pt-2">
  <h2 class="text-lg font-semibold tracking-tight">Accounts</h2>
  <p class="text-sm text-muted-foreground">
    Which profile each account is on.
  </p>
</header>

<section class="overflow-hidden rounded-lg border border-border">
  {#each accounts as account, index (account.puuid)}
    <div class="group flex min-h-14 items-center gap-3 px-4 py-2.5" class:border-t={index > 0}>
      {#if forgetting === account.puuid}
        <p class="min-w-0 flex-1 truncate text-sm">
          Forget <span class="font-medium">{account.name}</span>?
        </p>
        <Button variant="destructive" size="sm" onclick={() => forget(account)}>Forget</Button>
        <Button variant="ghost" size="sm" onclick={() => (forgetting = null)}>Cancel</Button>
      {:else}
        <div class="min-w-0 flex-1">
          <p class="flex items-center gap-2 truncate text-sm font-medium">
            {account.name}
            {#if account.connected}<Badge variant="secondary">logged in</Badge>{/if}
          </p>
          <p class="text-xs text-muted-foreground">
            {account.profile ? `On ${account.profile}` : "Not on a profile"}
          </p>
        </div>
        <label class="flex items-center gap-2 text-xs text-muted-foreground">
          Auto-apply
          <Switch
            checked={account.autoApply}
            disabled={!account.profile}
            onCheckedChange={(enabled) => setAutoApply(account, enabled)}
          />
        </label>
        <Button
          class="opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
          variant="ghost"
          size="icon"
          onclick={() => (forgetting = account.puuid)}
          aria-label="Forget account"
          title="Forget account"
        >
          <Trash2 />
        </Button>
      {/if}
    </div>
  {:else}
    <p class="px-4 py-10 text-center text-sm text-muted-foreground">None yet.</p>
  {/each}
</section>
