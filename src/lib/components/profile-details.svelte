<!-- What a profile holds, and what applying it to the logged-in account would change. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Bind from "$lib/components/bind.svelte";
  import ChangeList from "$lib/components/change-list.svelte";
  import { Input } from "$lib/components/ui/input";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import * as api from "$lib/api";
  import { isBind, settingLabel } from "$lib/binds";

  let { id }: { id: string } = $props();

  let details = $state<api.ProfileDetails | null>(null);
  let failure = $state<string | null>(null);
  let filter = $state("");

  async function refresh() {
    try {
      details = await api.getProfileDetails(id);
      failure = null;
    } catch (err) {
      failure = String(err);
    }
  }

  onMount(() => {
    refresh();
    return api.onViewChanged(refresh);
  });

  const shown = $derived(
    (details?.settings ?? []).filter((row) =>
      `${row.section} ${row.key} ${row.value}`.toLowerCase().includes(filter.trim().toLowerCase()),
    ),
  );
</script>

<div class="flex flex-col gap-3 border-t border-border bg-muted/30 px-4 py-3">
  {#if failure}
    <p class="text-xs text-destructive">{failure}</p>
  {:else if details}
    <div>
      <p class="pb-1 text-[0.625rem] font-medium tracking-wide text-muted-foreground uppercase">
        {#if details.preview === null}
          Log into League to see what applying would change
        {:else if details.preview.length === 0}
          Matches this account
        {:else}
          Applying would change {details.preview.length}
        {/if}
      </p>
      {#if details.preview?.length}
        <div class="overflow-hidden rounded-md border border-border bg-background">
          <ChangeList changes={details.preview} />
        </div>
      {/if}
    </div>

    <div>
      <div class="flex items-center gap-2 pb-1">
        <p class="flex-1 text-[0.625rem] font-medium tracking-wide text-muted-foreground uppercase">
          All {details.settings.length} settings
        </p>
        <Input class="h-6 w-40" bind:value={filter} placeholder="Filter…" />
      </div>
      <ScrollArea class="h-48 rounded-md border border-border bg-background">
        <ul class="divide-y divide-border text-xs">
          {#each shown as row (row.file + row.section + row.key)}
            <li class="flex items-center gap-2 px-2.5 py-1">
              <span class="w-24 shrink-0 truncate text-faint">{row.section}</span>
              <span class="min-w-0 flex-1 truncate">{settingLabel(row.key)}</span>
              {#if isBind(row)}
                <Bind value={row.value} />
              {:else}
                <span class="shrink-0 text-muted-foreground">{row.value || "none"}</span>
              {/if}
            </li>
          {:else}
            <li class="px-2.5 py-4 text-center text-faint">No match.</li>
          {/each}
        </ul>
      </ScrollArea>
    </div>
  {/if}
</div>
