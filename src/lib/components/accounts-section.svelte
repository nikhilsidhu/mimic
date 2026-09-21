<!-- Every account mimic has seen: which profile it is on, and whether it auto-applies. It
     folds out of the manager's header, which shows the account in use. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { Button } from "$lib/components/ui/button";
  import { Switch } from "$lib/components/ui/switch";
  import Confirm from "$lib/components/confirm.svelte";
  import Status from "$lib/components/status.svelte";
  import * as api from "$lib/api";
  import { attempt, type Report } from "$lib/attempt";

  let { onmessage }: { onmessage: Report } = $props();

  let accounts = $state<api.AccountRow[]>([]);
  /** The account asking to confirm being forgotten, by puuid. */
  let forgetting = $state<string | null>(null);

  const refresh = async () => (accounts = await api.getAccounts());

  onMount(() => {
    refresh();
    return api.onViewChanged(refresh);
  });

  async function setAutoApply(account: api.AccountRow, enabled: boolean) {
    await attempt(onmessage, () => api.setAccountAutoApply(account.puuid, enabled));
    refresh();
  }

  async function forget(account: api.AccountRow) {
    await attempt(onmessage, () => api.forgetAccount(account.puuid));
    forgetting = null;
    refresh();
  }
</script>

<section class="overflow-hidden rounded-lg border border-border">
  {#each accounts as account, index (account.puuid)}
    <div class="group flex min-h-14 items-center gap-3 px-4 py-2.5" class:border-t={index > 0}>
      {#if forgetting === account.puuid}
        <p class="min-w-0 flex-1 truncate text-sm">
          Forget <span class="font-medium">{account.name}</span>?
        </p>
        <Confirm action="Forget" onconfirm={() => forget(account)} oncancel={() => (forgetting = null)} />
      {:else}
        <div class="min-w-0 flex-1">
          <p class="flex h-5 items-center gap-2 truncate text-sm font-medium">
            {account.name}
            {#if account.connected}<Status>logged in</Status>{/if}
          </p>
          <p class="mt-0.5 text-xs text-muted-foreground">
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
          class="text-faint transition-colors group-hover:text-muted-foreground"
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
