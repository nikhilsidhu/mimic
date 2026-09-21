<!-- The prompt that comes up by itself, after a game or at login, when settings changed. -->
<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import ChangesReview from "$lib/components/changes-review.svelte";
  import * as api from "$lib/api";

  const close = () => getCurrentWindow().hide();
</script>

<main class="flex h-screen flex-col border border-border bg-popover p-4 text-popover-foreground">
  <ChangesReview
    onclose={close}
    ondone={(said) => {
      close();
      api.notify(said);
    }}
  />
</main>
