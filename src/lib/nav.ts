// Controller-first navigation, like a console UI. Everything focusable carries `data-nav`.
// Arrow keys and the d-pad/left stick move focus to the nearest element in that direction;
// A/Enter activates it, B/Escape goes back. Works on every screen without per-screen code.

export type Dir = "up" | "down" | "left" | "right";
export type PadAction = Dir | "a" | "b" | "x" | "y" | "lb" | "rb" | "start" | "select";

const visible = (el: HTMLElement) => {
  const r = el.getBoundingClientRect();
  return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== "hidden";
};

/** Navigable elements inside the topmost open layer (menu/search overlay) or the page. */
function candidates(): HTMLElement[] {
  const layers = document.querySelectorAll<HTMLElement>("[data-layer]");
  const layer = layers[layers.length - 1] ?? document.body;
  return [...layer.querySelectorAll<HTMLElement>("[data-nav]:not([disabled])")].filter(visible);
}

/** Focus where a screen starts: an element marked `data-nav-start` (the selected tab or
 *  filter), else its first control. */
export function focusFirst(scope?: Element | null) {
  const s = scope ?? document;
  const el = s.querySelector<HTMLElement>("[data-nav][data-nav-start]:not([disabled])") ?? s.querySelector<HTMLElement>("[data-nav]:not([disabled])");
  el?.focus({ preventScroll: true });
  el?.scrollIntoView({ block: "nearest", inline: "nearest" });
}

export function move(dir: Dir): boolean {
  const els = candidates();
  if (!els.length) return false;
  const cur = document.activeElement as HTMLElement | null;
  if (!cur || !els.includes(cur)) {
    els[0].focus({ preventScroll: true });
    els[0].scrollIntoView({ block: "nearest", inline: "nearest" });
    return true;
  }
  const a = cur.getBoundingClientRect();
  const ax = a.left + a.width / 2;
  const ay = a.top + a.height / 2;
  let best: HTMLElement | null = null;
  let bestScore = Infinity;
  for (const el of els) {
    if (el === cur) continue;
    const b = el.getBoundingClientRect();
    const bx = b.left + b.width / 2;
    const by = b.top + b.height / 2;
    let main: number, cross: number;
    switch (dir) {
      case "right": main = b.left - a.right; cross = Math.abs(by - ay); if (bx <= ax + 1) continue; break;
      case "left": main = a.left - b.right; cross = Math.abs(by - ay); if (bx >= ax - 1) continue; break;
      case "down": main = b.top - a.bottom; cross = Math.abs(bx - ax); if (by <= ay + 1) continue; break;
      case "up": main = a.top - b.bottom; cross = Math.abs(bx - ax); if (by >= ay - 1) continue; break;
    }
    // Overlapping rows/columns are strongly preferred over diagonal jumps.
    const score = Math.max(main, -20) + cross * 2.2;
    if (score < bestScore) {
      bestScore = score;
      best = el;
    }
  }
  if (!best) return false;
  best.focus({ preventScroll: true });
  const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  best.scrollIntoView({ block: "nearest", inline: "nearest", behavior: still ? "auto" : "smooth" });
  return true;
}

const BUTTONS: Record<number, PadAction> = {
  0: "a", 1: "b", 2: "x", 3: "y", 4: "lb", 5: "rb", 8: "select", 9: "start",
  12: "up", 13: "down", 14: "left", 15: "right",
};

/** Polls connected controllers and dispatches `pad` events; costs nothing with no pad. */
export function startGamepad(): () => void {
  let raf = 0;
  const held = new Map<PadAction, number>();
  const fire = (a: PadAction) => window.dispatchEvent(new CustomEvent<PadAction>("pad", { detail: a }));
  function poll(t: number) {
    const pads = (navigator.getGamepads?.() ?? []).filter(Boolean) as Gamepad[];
    if (!pads.length) {
      raf = 0;
      return;
    }
    const down = new Set<PadAction>();
    for (const p of pads) {
      p.buttons.forEach((b, i) => b.pressed && BUTTONS[i] && down.add(BUTTONS[i]));
      const [x = 0, y = 0] = p.axes;
      if (x < -0.55) down.add("left");
      if (x > 0.55) down.add("right");
      if (y < -0.55) down.add("up");
      if (y > 0.55) down.add("down");
    }
    // The pad is shared with whatever game is in front. Only act while OnionRing itself is the
    // focused window; anything held when focus returns is ignored until it is released.
    if (!document.hasFocus()) {
      for (const a of down) held.set(a, Infinity);
      for (const a of [...held.keys()]) if (!down.has(a)) held.delete(a);
      raf = requestAnimationFrame(poll);
      return;
    }
    for (const a of down) {
      const since = held.get(a);
      const repeats = a === "up" || a === "down" || a === "left" || a === "right";
      if (since === undefined) {
        held.set(a, t);
        fire(a);
      } else if (repeats && t - since > 360) {
        held.set(a, t - 360 + 100);
        fire(a);
      }
    }
    for (const a of [...held.keys()]) if (!down.has(a)) held.delete(a);
    raf = requestAnimationFrame(poll);
  }
  const kick = () => {
    if (!raf) raf = requestAnimationFrame(poll);
  };
  window.addEventListener("gamepadconnected", kick);
  kick();
  return () => {
    window.removeEventListener("gamepadconnected", kick);
    cancelAnimationFrame(raf);
  };
}
