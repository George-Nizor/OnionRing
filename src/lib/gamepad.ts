// Maps controller input onto a small set of actions, dispatched as `pad` events on window.
// Polling only runs while a pad is connected, so it costs nothing otherwise.

export type PadAction = "up" | "down" | "left" | "right" | "a" | "b" | "y" | "lb" | "rb" | "start";

const BUTTONS: Record<number, PadAction> = {
  0: "a",
  1: "b",
  3: "y",
  4: "lb",
  5: "rb",
  9: "start",
  12: "up",
  13: "down",
  14: "left",
  15: "right",
};
const REPEAT_DELAY = 380;
const REPEAT_RATE = 110;

export function startGamepad() {
  let raf = 0;
  const held = new Map<PadAction, number>();

  const fire = (a: PadAction) => window.dispatchEvent(new CustomEvent<PadAction>("pad", { detail: a }));

  function poll(t: number) {
    const pads = navigator.getGamepads().filter(Boolean) as Gamepad[];
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
    for (const a of down) {
      const since = held.get(a);
      const repeats = a === "up" || a === "down" || a === "left" || a === "right";
      if (since === undefined) {
        held.set(a, t);
        fire(a);
      } else if (repeats && t - since > REPEAT_DELAY) {
        held.set(a, t - REPEAT_DELAY + REPEAT_RATE);
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
