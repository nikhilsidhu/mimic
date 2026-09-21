<!-- Champions with settings of their own: a card each, read far more often than changed. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import AddChampion from "$lib/components/add-champion.svelte";
  import FlareImage from "$lib/components/flare-image.svelte";
  import FromTo from "$lib/components/from-to.svelte";
  import Confirm from "$lib/components/confirm.svelte";
  import SectionHeader from "$lib/components/section-header.svelte";
  import Status from "$lib/components/status.svelte";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import * as api from "$lib/api";
  import { attempt, type Report } from "$lib/attempt";
  import { isBind, settingLabel } from "$lib/binds";

  type Props = { view: api.View | null; onmessage: Report; onchanged: () => void };
  let { view, onmessage, onchanged }: Props = $props();

  // A card shows this many overrides until it is asked for the rest, so that one champion
  // with many does not push the others off the screen.
  const CARD_ROWS = 4;
  // With this many champions a filter appears.
  const FILTER_FROM = 9;

  let section = $state<HTMLElement | null>(null);
  let adding = $state(false);
  let busy = $state(false);
  let filter = $state("");
  let unfolded = $state<number[]>([]);
  /** The champion whose card shows its remove and delete buttons. */
  let editingCard = $state<number | null>(null);
  /** The champion being asked about before it is deleted. */
  let deleting = $state<number | null>(null);
  /** The override being asked about before it is removed, as `champion/setting`. */
  let removing = $state<string | null>(null);

  const champions = $derived(
    (view?.overlays ?? []).filter((overlay) => overlay.champion.name.toLowerCase().includes(filter.trim().toLowerCase())),
  );

  // The tray panel sends people here to save settings for a champion.
  onMount(() =>
    api.onAddChampion(async () => {
      adding = true;
      await tick();
      section?.scrollIntoView({ behavior: "smooth", block: "start" });
    }),
  );

  async function run(action: () => Promise<string>) {
    if (busy) return;
    busy = true;
    await attempt(onmessage, action);
    busy = false;
    deleting = null;
    removing = null;
    onchanged();
  }
</script>

<div bind:this={section} class="scroll-mt-4">
  <SectionHeader title="Champions" description="Overrides used only while you play that champion.">
    {#if (view?.overlays.length ?? 0) >= FILTER_FROM}
      <Input class="h-6 w-36" bind:value={filter} placeholder="Filter…" />
    {/if}
    {#if !adding}
      <Button variant="outline" size="sm" onclick={() => (adding = true)}><Plus />Add champion</Button>
    {/if}
  </SectionHeader>
</div>

{#if adding}
  <AddChampion
    against={view?.profiles.find((profile) => profile.active)?.name ?? null}
    oncancel={() => (adding = false)}
    onsaved={(said) => {
      adding = false;
      onmessage(said, false);
      onchanged();
    }}
  />
{/if}


<!-- A card per champion, two to a row: narrow lists keep each value next to its name
     and are read top to bottom. -->
<section class="grid gap-3 min-[720px]:grid-cols-2">
  {#each champions as overlay (overlay.champion.id)}
    {@const id = overlay.champion.id}
    <!-- Editing a card shows all of it: what is to be removed may be below the fold. -->
    {@const folded = overlay.settings.length > CARD_ROWS && !unfolded.includes(id) && editingCard !== id}
    <div class="group rounded-lg border border-border px-3 pt-2.5 pb-2">
      <div class="flex h-8 items-center gap-2.5">
        <FlareImage src={overlay.champion.icon} class="size-7 rounded-md" />
        {#if deleting === id}
          <p class="min-w-0 flex-1 truncate text-sm">Delete {overlay.champion.name}?</p>
          <Confirm action="Delete" disabled={busy} onconfirm={() => run(() => api.deleteOverlay(id))} oncancel={() => (deleting = null)} />
        {:else}
          <p class="min-w-0 truncate text-sm font-medium">{overlay.champion.name}</p>
          {#if view?.activeOverlay?.id === overlay.champion.id}<Status>active</Status>{/if}
          <span class="flex-1"></span>
          <!-- A card is read far more often than it is changed, so what changes it shows
               only once asked for. Nothing depends on hovering. -->
          {#if editingCard === overlay.champion.id}
            <Button
              class="hover-danger text-muted-foreground"
              variant="ghost"
              size="icon-sm"
              onclick={() => (deleting = id)}
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
          <li class="relative grid min-h-7.5 items-center gap-2 {editingCard === overlay.champion.id ? 'grid-cols-[minmax(0,1fr)_auto_auto]' : 'grid-cols-[minmax(0,1fr)_auto]'}">
            <span class="pr-2 leading-tight text-muted-foreground" title="{setting.section} / {setting.key}">
              {settingLabel(setting.key)}
            </span>
            <!-- What the champion changes it from, when that is known, and to. -->
            <FromTo keys={isBind(setting)} name={setting.key} from={setting.from ?? undefined} to={setting.value} />
            {#if removing === `${id}/${api.muteId(setting)}`}
              <span class="absolute inset-y-0 right-0 flex items-center gap-1 bg-linear-to-l from-background from-85% to-transparent pl-8">
                <Confirm
                  action="Remove"
                  disabled={busy}
                  onconfirm={() => run(() => api.removeOverlaySetting(id, setting))}
                  oncancel={() => (removing = null)}
                />
              </span>
            {:else if editingCard === overlay.champion.id}
              <span class="flex items-center">
                <button
                  class="icon-glow hover-danger -mr-1 flex size-6 items-center justify-center rounded text-muted-foreground"
                  disabled={busy}
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
