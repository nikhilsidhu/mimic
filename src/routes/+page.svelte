<script lang="ts">
  import { onMount } from "svelte";
  import Check from "@lucide/svelte/icons/check";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import Titlebar from "$lib/components/titlebar.svelte";
  import * as api from "$lib/api";

  let view = $state<api.View | null>(null);
  let busy = $state<string | null>(null);
  let message = $state<{ text: string; failed: boolean } | null>(null);

  const refresh = async () => (view = await api.getView());

  onMount(() => {
    refresh();
    return api.onViewChanged(refresh);
  });

  async function apply(id: string) {
    if (busy) return;
    busy = id;
    try {
      message = { text: await api.applyProfile(id), failed: false };
    } catch (err) {
      message = { text: String(err), failed: true };
    } finally {
      busy = null;
      refresh();
    }
  }
</script>

<div class="flex h-screen flex-col bg-background">
  <Titlebar />

  <ScrollArea class="min-h-0 flex-1">
    <main class="mx-auto flex max-w-2xl flex-col gap-6 px-8 pt-6 pb-10">
      <header class="flex items-end justify-between gap-4">
        <div class="min-w-0">
          <h1 class="text-lg font-semibold tracking-tight">Profiles</h1>
          <p class="truncate text-sm text-muted-foreground">{view?.status ?? "Starting…"}</p>
        </div>
        {#if view?.phase && view.phase !== "None"}
          <Badge variant="secondary">{view.phase}</Badge>
        {/if}
      </header>

      <section class="overflow-hidden rounded-lg border border-border">
        {#each view?.profiles ?? [] as profile, index (profile.id)}
          <div class="flex items-center gap-3 px-4 py-3" class:border-t={index > 0}>
            <span class="flex size-4 shrink-0 items-center justify-center">
              {#if profile.active}<Check class="size-4" />{/if}
            </span>
            <div class="min-w-0 flex-1">
              <p class="truncate text-sm font-medium">{profile.name}</p>
              <p class="text-xs text-muted-foreground">{profile.settings} settings</p>
            </div>
            <Button
              variant="outline"
              size="sm"
              disabled={!view?.connected || busy !== null}
              onclick={() => apply(profile.id)}
            >
              {busy === profile.id ? "Applying…" : "Apply"}
            </Button>
          </div>
        {:else}
          <p class="px-4 py-10 text-center text-sm text-muted-foreground">
            No profiles yet. Log into League, then save your current settings from the tray.
          </p>
        {/each}
      </section>

      {#if message}
        <p class="text-sm" class:text-destructive={message.failed} class:text-muted-foreground={!message.failed}>
          {message.text}
        </p>
      {/if}
    </main>
  </ScrollArea>
</div>
