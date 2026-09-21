// The glow of icon buttons, after two things in ghost: the blurred duplicate behind its mic icon,
// which makes a glow in the icon's own shape, and the flare on its avatars, a spot of light that
// follows the pointer. Here the spot reveals the duplicate, so the light comes from the icon's
// strokes and is strongest where the pointer is.
//
// One set of listeners for the whole window. The duplicate exists only while the pointer is over
// the button, so that nothing is left in markup the framework owns.

const LAYER = "icon-glow-layer";

function button(event: Event): HTMLElement | null {
  return event.target instanceof Element ? event.target.closest<HTMLElement>(".icon-glow") : null;
}

function enter(event: PointerEvent) {
  const host = button(event);
  const icon = host?.querySelector<SVGElement>(":scope > svg");
  if (!host || !icon || host.querySelector(`.${LAYER}`)) return;
  // A wrapper the size of the button, so that the spot is placed in the button's coordinates,
  // which is what the pointer's position is measured in.
  const layer = document.createElement("span");
  layer.className = LAYER;
  layer.setAttribute("aria-hidden", "true");
  layer.append(icon.cloneNode(true));
  host.prepend(layer);
  follow(event);
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
  host.querySelector(`.${LAYER}`)?.remove();
}

/** Starts lighting icon buttons under the pointer. Returns a function that stops. */
export function watchGlow(): () => void {
  window.addEventListener("pointerover", enter, { passive: true });
  window.addEventListener("pointermove", follow, { passive: true });
  window.addEventListener("pointerout", leave, { passive: true });
  return () => {
    window.removeEventListener("pointerover", enter);
    window.removeEventListener("pointermove", follow);
    window.removeEventListener("pointerout", leave);
  };
}
