// The colour an avatar glows in, taken from the avatar itself, and the handlers that keep the
// glow at the pointer. Ported from ghost (github.com/nikhilsidhu/ghost, `lib/gradients.ts`), where
// it lights the avatars in the sidebar.
//
// OKLCH is a colour space where lightness, saturation and hue are independent axes, which lets a
// colour be brightened without washing it out, unlike blending toward white in RGB.
// Conversion chain: sRGB (0-255) -> linear RGB -> OKLab -> OKLCH, and back.

const GLOW_MIN_LIGHTNESS = 0.65;
const GLOW_CHROMA_BOOST = 1.6;
const GLOW_MAX_CHROMA = 0.25;
const SAMPLE_SIZE = 64;
const RING_WIDTH = 5;
const MIN_CHROMA = 5;

function srgbToLinear(c: number): number {
  const v = c / 255;
  return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
}

function linearToSrgb(c: number): number {
  const v = Math.max(0, Math.min(1, c));
  return v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055;
}

function rgbToOklch(r: number, g: number, b: number): [number, number, number] {
  const lr = srgbToLinear(r), lg = srgbToLinear(g), lb = srgbToLinear(b);
  const l_ = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb);
  const m_ = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb);
  const s_ = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb);
  const L = 0.2104542553 * l_ + 0.793617785 * m_ - 0.0040720468 * s_;
  const a = 1.9779984951 * l_ - 2.428592205 * m_ + 0.4505937099 * s_;
  const ob = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.808675766 * s_;
  return [L, Math.sqrt(a * a + ob * ob), ((Math.atan2(ob, a) * 180) / Math.PI + 360) % 360];
}

function oklchToRgb(L: number, C: number, H: number): [number, number, number] {
  const hRad = (H * Math.PI) / 180;
  const a = C * Math.cos(hRad), ob = C * Math.sin(hRad);
  const l_ = L + 0.3963377774 * a + 0.2158037573 * ob;
  const m_ = L - 0.1055613458 * a - 0.0638541728 * ob;
  const s_ = L - 0.0894841775 * a - 1.291485548 * ob;
  const l = l_ * l_ * l_, m = m_ * m_ * m_, s = s_ * s_ * s_;
  return [
    Math.round(linearToSrgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s) * 255),
    Math.round(linearToSrgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s) * 255),
    Math.round(linearToSrgb(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s) * 255),
  ];
}

/** Raises lightness and boosts chroma, for a vivid glow from any source colour. */
function liftToGlow(r: number, g: number, b: number): [number, number, number] {
  const [lightness, chroma, hue] = rgbToOklch(r, g, b);
  const L = Math.max(lightness, GLOW_MIN_LIGHTNESS + (1 - GLOW_MIN_LIGHTNESS) * lightness);
  return oklchToRgb(L, Math.min(chroma * GLOW_CHROMA_BOOST, GLOW_MAX_CHROMA), hue);
}

const toHex = (n: number) => Math.round(Math.min(255, Math.max(0, n))).toString(16).padStart(2, "0");

/** The 5px ring at the circular edge, weighted by how colourful each pixel is, brightened for a dark ground. */
function edgeRingGlow(data: Uint8ClampedArray): string | null {
  const center = SAMPLE_SIZE / 2;
  const radius = SAMPLE_SIZE / 2;
  const inner = radius - RING_WIDTH;
  let total = 0, red = 0, green = 0, blue = 0;

  for (let y = 0; y < SAMPLE_SIZE; y++) {
    for (let x = 0; x < SAMPLE_SIZE; x++) {
      const dist = Math.sqrt((x - center) ** 2 + (y - center) ** 2);
      if (dist < inner || dist > radius) continue;
      const i = (y * SAMPLE_SIZE + x) * 4;
      if (data[i + 3] < 128) continue;
      const chroma = Math.max(data[i], data[i + 1], data[i + 2]) - Math.min(data[i], data[i + 1], data[i + 2]);
      if (chroma < MIN_CHROMA) continue;
      const weight = chroma * chroma;
      red += data[i] * weight;
      green += data[i + 1] * weight;
      blue += data[i + 2] * weight;
      total += weight;
    }
  }

  if (total < 1) return null;
  const [r, g, b] = liftToGlow(red / total, green / total, blue / total);
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

/**
 * The glow colour of a loaded image, or null when it has none to speak of or cannot be read: the
 * icon comes from another origin, and is only readable if it was loaded with `crossorigin`.
 */
export function extractGlowColor(img: HTMLImageElement): string | null {
  const canvas = document.createElement("canvas");
  canvas.width = SAMPLE_SIZE;
  canvas.height = SAMPLE_SIZE;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  try {
    ctx.drawImage(img, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
    return edgeRingGlow(ctx.getImageData(0, 0, SAMPLE_SIZE, SAMPLE_SIZE).data);
  } catch {
    return null;
  }
}

/** Keeps the flare of an `.avatar-flare` element at the pointer. */
export function onFlareMove(event: PointerEvent) {
  const el = event.currentTarget as HTMLElement;
  const box = el.getBoundingClientRect();
  el.style.setProperty("--flare-x", `${((event.clientX - box.left) / box.width) * 100}%`);
  el.style.setProperty("--flare-y", `${((event.clientY - box.top) / box.height) * 100}%`);
  el.style.setProperty("--flare-opacity", "1");
}

export function onFlareLeave(event: PointerEvent) {
  (event.currentTarget as HTMLElement).style.setProperty("--flare-opacity", "0");
}
