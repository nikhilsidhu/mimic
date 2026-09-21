<!--
  Gives a champion its own settings in two steps: pick the champion, then pick where its
  settings come from. A champion's settings are stored as differences from the account's
  base, so every source shows how many settings it would override.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import * as api from "$lib/api";

  // `against` names what the account is on, which the sources are compared with.
  type Props = { against: string | null; onsaved: (said: string) => void; oncancel: () => void };
  let { against, onsaved, oncancel }: Props = $props();

  let champions = $state<api.Champion[]>([]);
  let sources = $state<api.OverlaySource[] | null>(null);
  /** Why there are no sources, e.g. nobody is logged in. */
  let unavailable = $state<string | null>(null);
  let search = $state("");
  let champion = $state<api.Champion | null>(null);
  let busy = $state(false);
  let failure = $state<string | null>(null);
  let searchInput = $state<HTMLInputElement | null>(null);

  // The champions this account plays come first, most played at the top; the backend
  // already sorted by name, and the sort is stable.
  const matches = $derived(
    champions
      .filter((candidate) => candidate.name.toLowerCase().includes(search.trim().toLowerCase()))
      .toSorted((a, b) => b.mastery - a.mastery),
  );

  onMount(async () => {
    champions = await api.getChampions();
    searchInput?.focus();
    try {
      sources = await api.getOverlaySources();
    } catch (err) {
      unavailable = String(err);
    }
  });

  async function save(source: api.OverlaySource) {
    if (!champion || busy) return;
    busy = true;
    failure = null;
    try {
      onsaved(await api.saveOverlay(champion.id, source.profile));
    } catch (err) {
      failure = String(err);
    } finally {
      busy = false;
    }
  }

  const count = (settings: number) => (settings === 1 ? "1 setting" : `${settings} settings`);
</script>

<section class="list-box">
  {#if !champion}
    <div class="flex items-center gap-2 border-b border-border p-2">
      <Input
        bind:ref={searchInput}
        bind:value={search}
        placeholder="Search champions…"
        onkeydown={(event) => {
          if (event.key === "Escape") oncancel();
          if (event.key === "Enter" && matches.length === 1) champion = matches[0];
        }}
      />
      <Button variant="ghost" size="sm" onclick={oncancel}>Cancel</Button>
    </div>
    <ScrollArea class="h-56">
      <div class="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-0.5 p-1.5">
        {#each matches as candidate (candidate.id)}
          <button
            class="flex items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent"
            onclick={() => (champion = candidate)}
          >
            <img src={candidate.icon} alt="" loading="lazy" class="size-6 shrink-0 rounded-sm" />
            <span class="truncate">{candidate.name}</span>
          </button>
        {:else}
          <p class="col-span-full px-2 py-8 text-center text-sm text-muted-foreground">
            {champions.length ? "No match." : "Open League once to load champions."}
          </p>
        {/each}
      </div>
    </ScrollArea>
  {:else}
    <div class="flex items-center gap-3 border-b border-border px-4 py-2.5">
      <img src={champion.icon} alt="" class="size-8 shrink-0 rounded-md" />
      <div class="min-w-0 flex-1">
        <p class="truncate text-sm font-medium">{champion.name}</p>
        <p class="text-xs text-muted-foreground">Take its settings from:</p>
      </div>
      <Button variant="ghost" size="sm" onclick={() => (champion = null)}>Back</Button>
      <Button variant="ghost" size="sm" onclick={oncancel}>Cancel</Button>
    </div>

    {#if unavailable}
      <p class="px-4 py-6 text-sm text-muted-foreground">
        Log into League first.
        <span class="text-destructive">{unavailable}</span>
      </p>
    {:else}
      {#each sources ?? [] as source, index (source.profile ?? "")}
        <button
          class="flex w-full items-center gap-3 px-4 py-2.5 text-left hover:bg-accent disabled:opacity-50 disabled:hover:bg-transparent"
          class:border-t={index > 0}
          disabled={busy || source.settings === 0}
          onclick={() => save(source)}
        >
          <div class="min-w-0 flex-1">
            {#if source.profile === null}
              <p class="text-sm font-medium">What changed just now</p>
              <p class="text-xs text-muted-foreground">
                {source.settings
                  ? `${count(source.settings)}, moved to ${champion.name}`
                  : "Nothing has changed"}
              </p>
            {:else}
              <p class="truncate text-sm font-medium">From {source.name}</p>
              <p class="text-xs text-muted-foreground">{count(source.settings)} that differ from {against ?? "this account"}</p>
            {/if}
          </div>
          <ChevronRight class="size-4 shrink-0 text-muted-foreground" />
        </button>
      {/each}
    {/if}

    {#if failure}
      <p class="border-t border-border px-4 py-2 text-sm text-destructive">{failure}</p>
    {/if}
  {/if}
</section>
