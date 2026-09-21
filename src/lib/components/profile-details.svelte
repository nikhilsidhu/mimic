<!-- What a profile holds, and what applying it to the logged-in account would change. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Bind from "$lib/components/bind.svelte";
  import ChangeList from "$lib/components/change-list.svelte";
  import { Input } from "$lib/components/ui/input";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import * as api from "$lib/api";
  import { isBind, isBound, sectionLabel, settingLabel, valueLabel } from "$lib/binds";

  let { id }: { id: string } = $props();

  let details = $state<api.ProfileDetails | null>(null);
  let failure = $state<string | null>(null);
  let filter = $state("");
  /** Most keys a profile lists are bound to nothing; they stay out of the way until asked for. */
  let showUnbound = $state(false);

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

  const wanted = $derived(filter.trim().toLowerCase());
  // Found by the name shown as well as Riot's own, so that either can be typed.
  const matches = (row: api.SettingRow) =>
    `${sectionLabel(row.section)} ${settingLabel(row.key)} ${row.section} ${row.key} ${row.value}`.toLowerCase().includes(wanted);

  const binds = $derived((details?.settings ?? []).filter(isBind));
  const unbound = $derived(binds.filter((row) => !isBound(row.value)).length);
  const shownBinds = $derived(binds.filter((row) => matches(row) && (showUnbound || wanted !== "" || isBound(row.value))));

  /** Everything that is not a keybind, under the heading League's menus would put it. */
  const groups = $derived.by(() => {
    const byHeading = new Map<string, api.SettingRow[]>();
    for (const row of details?.settings ?? []) {
      if (isBind(row) || !matches(row)) continue;
      const heading = sectionLabel(row.section);
      byHeading.set(heading, [...(byHeading.get(heading) ?? []), row]);
    }
    return [...byHeading].sort(([a], [b]) => a.localeCompare(b));
  });
  const others = $derived(groups.reduce((total, [, rows]) => total + rows.length, 0));

  const heading = "text-[0.625rem] font-medium tracking-wide text-muted-foreground uppercase";
</script>

<div class="flex flex-col gap-3 border-t border-border bg-muted/30 px-4 py-3">
  {#if failure}
    <p class="text-xs text-destructive">{failure}</p>
  {:else if details}
    <!-- How many would change is said on the profile's row; here is which. -->
    {#if details.preview?.length}
      <div>
        <p class="pb-1 {heading}">Would change</p>
        <div class="max-h-56 overflow-y-auto rounded-md border border-border bg-background">
          <ChangeList changes={details.preview} />
        </div>
      </div>
    {/if}

    <!-- Keys on one side, everything else on the other, so that neither is a list of hundreds. -->
    <div class="grid grid-cols-2 gap-3">
      <div class="min-w-0">
        <!-- What only concerns the keys sits in their heading: most are bound to nothing. -->
        <div class="flex h-6 items-center gap-2 pb-1">
          <p class="flex-1 {heading}">Keys · {shownBinds.length}</p>
          <label class="flex items-center gap-1.5 text-xs text-muted-foreground">
            <input type="checkbox" class="accent-emerald-600" bind:checked={showUnbound} />
            {unbound} unbound
          </label>
        </div>
        <ScrollArea class="h-64 rounded-md border border-border bg-background">
          <ul class="divide-y divide-border text-xs">
            {#each shownBinds as row (row.file + row.section + row.key)}
              <li class="flex min-h-8 items-center gap-2 px-2.5" title="{row.section} / {row.key}">
                <span class="min-w-0 flex-1 truncate">{settingLabel(row.key)}</span>
                <Bind value={row.value} />
              </li>
            {:else}
              <li class="px-2.5 py-4 text-center text-faint">No match.</li>
            {/each}
          </ul>
        </ScrollArea>
      </div>

      <div class="min-w-0">
        <!-- The filter narrows both lists; it sits where a search usually does, at the far end. -->
        <div class="flex h-6 items-center gap-2 pb-1">
          <p class="flex-1 {heading}">Other settings · {others}</p>
          <Input class="h-5 w-32 text-xs" bind:value={filter} placeholder="Filter…" />
        </div>
        <ScrollArea class="h-64 rounded-md border border-border bg-background">
          {#each groups as [name, rows] (name)}
            <p class="sticky top-0 border-b border-border bg-background px-2.5 py-1 text-xs font-medium">{name}</p>
            <ul class="divide-y divide-border text-xs">
              {#each rows as row (row.file + row.section + row.key)}
                <li class="flex min-h-7 items-center gap-2 px-2.5" title="{row.section} / {row.key}">
                  <span class="min-w-0 flex-1 truncate text-muted-foreground">{settingLabel(row.key)}</span>
                  <span class="shrink-0">{valueLabel(row.key, row.value)}</span>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="px-2.5 py-4 text-center text-xs text-faint">No match.</p>
          {/each}
        </ScrollArea>
      </div>
    </div>
  {/if}
</div>
