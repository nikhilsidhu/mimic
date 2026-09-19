<!--
  The tray icon's right-click menu. Windows cannot style native menus, so this is a
  frameless window made to behave like one. Its height is set from Rust before it
  opens; the sizes below mirror the MENU_* constants in `src-tauri/src/tray.rs`.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import Check from "@lucide/svelte/icons/check";
  import * as api from "$lib/api";

  let view = $state<api.View | null>(null);
  const refresh = async () => (view = await api.getView());

  onMount(() => {
    refresh();
    const stop = api.onViewChanged(refresh);
    const onKey = (event: KeyboardEvent) => event.key === "Escape" && api.dismiss();
    window.addEventListener("keydown", onKey);
    return () => {
      stop();
      window.removeEventListener("keydown", onKey);
    };
  });

  /** Menus close on click; whatever the action has to say goes to the notice popup. */
  async function choose(action: () => Promise<string | void>) {
    api.dismiss();
    try {
      const said = await action();
      if (said) api.notify(said);
    } catch (err) {
      api.notify(String(err));
    }
  }
</script>

<main>
  {#if view?.profiles.length}
    <p class="label">Apply profile</p>
    <div class="profiles">
      {#each view.profiles as profile (profile.id)}
        <button onclick={() => choose(() => api.applyProfile(profile.id))}>
          <span class="mark">{#if profile.active}<Check class="size-3.5" />{/if}</span>
          <span class="name">{profile.name}</span>
        </button>
      {/each}
    </div>
    <hr />
  {/if}
  <button onclick={() => choose(api.openManager)}><span class="mark"></span>Open manager</button>
  <button onclick={() => choose(api.openLogs)}><span class="mark"></span>Open logs folder</button>
  <hr />
  <button onclick={api.quit}><span class="mark"></span>Quit mimic</button>
</main>

<style>
  main {
    box-sizing: border-box;
    height: 100vh;
    padding: 6px;
    border: 1px solid var(--border);
    background: var(--popover);
    color: var(--popover-foreground);
    font-size: 12px;
  }

  .label {
    height: 24px;
    padding: 0 8px;
    line-height: 24px;
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted-foreground);
  }

  /* Eight rows, then it scrolls. */
  .profiles {
    max-height: calc(28px * 8);
    overflow-y: auto;
  }

  button {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 28px;
    padding: 0 8px;
    border-radius: 6px;
    text-align: left;
  }

  button:hover:not(:disabled) {
    background: var(--accent);
  }

  button:disabled {
    opacity: 0.5;
  }

  .mark {
    display: flex;
    flex: none;
    width: 14px;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  hr {
    height: 1px;
    margin: 4px 0;
    border: 0;
    background: var(--border);
  }
</style>
