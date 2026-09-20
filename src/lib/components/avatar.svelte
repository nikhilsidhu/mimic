<!-- The connected account's profile icon with its level, or a neutral placeholder. -->
<script lang="ts">
  import UserRound from "@lucide/svelte/icons/user-round";

  let { account, size = "sm" }: { account: { icon: string; level: number } | null | undefined; size?: "sm" | "lg" } =
    $props();

  // The icon is fetched in the background at login, so it can be missing for a moment.
  let failed = $state(false);
  $effect(() => {
    account?.icon;
    failed = false;
  });
</script>

<div class="relative shrink-0 {size === 'lg' ? 'size-11' : 'size-8'}" title={account ? `Level ${account.level}` : undefined}>
  {#if account && !failed}
    <img src={account.icon} alt="" class="size-full rounded-full ring-1 ring-border" onerror={() => (failed = true)} />
    {#if size === "lg"}
      <span
        class="absolute -bottom-1 left-1/2 -translate-x-1/2 rounded-full border border-border bg-background px-1.5 text-[0.625rem] leading-4 text-muted-foreground"
      >
        {account.level}
      </span>
    {/if}
  {:else}
    <div class="flex size-full items-center justify-center rounded-full bg-muted text-muted-foreground ring-1 ring-border">
      <UserRound class={size === "lg" ? "size-5" : "size-4"} />
    </div>
  {/if}
</div>
