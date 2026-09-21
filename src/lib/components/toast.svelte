<!-- What the last action had to say, pinned to the bottom of the window so it is seen
     wherever the page is scrolled to. Errors stay until dismissed. -->
<script lang="ts">
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import X from "@lucide/svelte/icons/x";
  import { fly } from "svelte/transition";

  type Message = { text: string; failed: boolean };

  let { message = $bindable() }: { message: Message | null } = $props();

  /** How long a success stays. */
  const STAYS = 4000;

  $effect(() => {
    if (!message || message.failed) return;
    const shown = message;
    const timer = setTimeout(() => {
      if (message === shown) message = null;
    }, STAYS);
    return () => clearTimeout(timer);
  });
</script>

{#if message}
  <div class="pointer-events-none fixed inset-x-0 bottom-4 z-50 flex justify-center px-4">
    <div
      class="pointer-events-auto flex max-w-lg items-start gap-2 rounded-lg border bg-popover py-2 pr-2 pl-3 text-sm text-popover-foreground"
      class:border-border={!message.failed}
      class:border-destructive={message.failed}
      role={message.failed ? "alert" : "status"}
      transition:fly={{ y: 8, duration: 150 }}
    >
      {#if message.failed}<CircleAlert class="mt-0.5 size-4 shrink-0 text-destructive" />{/if}
      <p class="min-w-0 flex-1 break-words">{message.text}</p>
      <button
        class="icon-glow shrink-0 rounded p-0.5 text-faint hover:text-hover"
        aria-label="Dismiss"
        onclick={() => (message = null)}
      >
        <X class="size-4" />
      </button>
    </div>
  </div>
{/if}
