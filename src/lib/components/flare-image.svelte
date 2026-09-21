<!-- An icon that glows in its own colour under the pointer, as the avatars in ghost do: the
     account's icon, a champion's. The colour is taken from the image's edge once it has loaded. -->
<script lang="ts">
  import { extractGlowColor, onFlareLeave, onFlareMove } from "$lib/avatar-glow";

  type Props = {
    src: string;
    /** Size and rounding, which the glow follows. */
    class?: string;
    onerror?: () => void;
  };
  let { src, class: shape = "", onerror }: Props = $props();

  /** The hover colour until the image's own is known, or if it cannot be read. */
  let glow = $state<string | null>(null);
  $effect(() => {
    src;
    glow = null;
  });
</script>

<span
  class="avatar-flare relative block shrink-0 {shape}"
  style:--glow-color={glow ?? "var(--hover)"}
  onpointermove={onFlareMove}
  onpointerleave={onFlareLeave}
  role="presentation"
>
  <!-- `crossorigin`: the icon comes from the asset protocol, another origin, and its pixels can
       only be read for the glow colour if it was loaded this way. -->
  <img
    {src}
    alt=""
    crossorigin="anonymous"
    class="size-full rounded-[inherit]"
    onload={(event) => (glow = extractGlowColor(event.currentTarget as HTMLImageElement))}
    {onerror}
  />
</span>
