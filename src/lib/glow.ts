// The glow of icon buttons, after two things in ghost: the blurred duplicate behind its mic icon,
// which makes a glow in the icon's own shape, and the flare on its avatars, a spot of light that
// follows the pointer. Here the spot reveals the duplicate, so the light comes from the icon's
// strokes and is strongest where the pointer is.
//
// One set of listeners for the whole window. The duplicate exists only while the pointer is over
// the button, so that nothing is left in markup the framework owns.

const LAYER = "icon-glow-layer";

/** Watches the hovered button for its icon being swapped, as Copy is for a tick once clicked. */
let swaps: MutationObserver | null = null;

function button(event: Event): HTMLElement | null {
  return event.target instanceof Element ? event.target.closest<HTMLElement>(".icon-glow") : null;
}

/** Puts a copy of the button's icon behind it, replacing any earlier copy. */
function light(host: HTMLElement) {
  host.querySelector(`:scope > .${LAYER}`)?.remove();
  const icon = host.querySelector<SVGElement>(":scope > svg");
  if (!icon) return;
  // A wrapper the size of the button, so that the spot is placed in the button's coordinates,
  // which is what the pointer's position is measured in.
  const layer = document.createElement("span");
  layer.className = LAYER;
  layer.setAttribute("aria-hidden", "true");
  layer.append(icon.cloneNode(true));
  host.prepend(layer);
}

function enter(event: PointerEvent) {
  const host = button(event);
  if (!host || host.querySelector(`:scope > .${LAYER}`)) return;
  light(host);
  follow(event);
  // The copy has the old icon's shape and colour; a new icon gets a new copy. Its own coming
  // and going are changes to the button's children too, and are not to be answered.
  swaps?.disconnect();
  swaps = new MutationObserver((changes) => {
    const ours = (node: Node) => node instanceof Element && node.classList.contains(LAYER);
    const swapped = changes.some((change) => [...change.addedNodes, ...change.removedNodes].some((node) => !ours(node)));
    if (swapped) light(host);
  });
  swaps.observe(host, { childList: true });
}

function follow(event: PointerEvent) {
  const host = button(event);
  if (!host) return;
  const box = host.getBoundingClientRect();
  host.style.setProperty("--glow-x", `${event.clientX - box.left}px`);
  host.style.setProperty("--glow-y", `${event.clientY - box.top}px`);
}

function leave(event: PointerEvent) {
  const host = button(event);
  // `pointerout` also fires when moving between the button's own children.
  if (!host || (event.relatedTarget instanceof Node && host.contains(event.relatedTarget))) return;
  swaps?.disconnect();
  swaps = null;
  host.querySelector(`:scope > .${LAYER}`)?.remove();
}

/** Starts lighting icon buttons under the pointer. Returns a function that stops. */
export function watchGlow(): () => void {
  window.addEventListener("pointerover", enter, { passive: true });
  window.addEventListener("pointermove", follow, { passive: true });
  window.addEventListener("pointerout", leave, { passive: true });
  return () => {
    swaps?.disconnect();
    window.removeEventListener("pointerover", enter);
    window.removeEventListener("pointermove", follow);
    window.removeEventListener("pointerout", leave);
  };
}
