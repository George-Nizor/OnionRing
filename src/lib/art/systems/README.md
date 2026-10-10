# Console art: format and style guide

One hand-made illustration per platform, in two styles. The pixel theme draws the 1-bit bitmap,
the paper theme draws the ink illustration (`src/lib/components/SystemArt.svelte`). Tiles show the
art at roughly 190 x 120 px; the platform page shows it larger. It has to read at tile size first.

The bar is set by `ps2.json` (dark plastic, wide and flat) and `gc.json` (coloured plastic, a cube
with round details). Put yours next to them on the contact sheet before you call it done.

## File format

`src/lib/art/systems/<platformId>.json`, one file per platform id from `crates/core/data/platforms.json`:

```json
{
  "pixel": ["................................................", "... 32 rows in total ..."],
  "ink": "<g fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'>...</g>"
}
```

- `pixel`: exactly 32 strings, each exactly 48 characters, only `#` (on) and `.` (off).
- `ink`: SVG markup for `viewBox="0 0 240 160"`, one root `<g>`. Use **single quotes** for every
  attribute so the JSON string stays valid.
- Allowed in ink: `g path rect circle ellipse line polyline polygon`, with `fill stroke stroke-*
  opacity rx transform` (rotate/translate only). Not allowed: `id`, `defs`, `clipPath`, `mask`,
  gradients, filters, `<text>`, `<image>`, `<style>`, `class`, external references. The same drawing
  can be on screen many times, so anything with an id would collide.
- Colours: `currentColor` for ink lines, `var(--panel)` for white/light-grey plastic and highlights,
  plus plain hex fills (rules below). No other CSS variables.
- Keep the ink string under about 12 KB: round numbers to one decimal, use `<ellipse>` and
  `<rect rx>` instead of sampled curves.

Check it: `node scripts/render-art.mjs <id>` writes `.art-preview/<id>-pixel.png` and
`<id>-ink.png` (and validates the grid). `node scripts/render-art.mjs --sheet` writes contact sheets
of every console drawn so far. Look at the PNGs, then look at them small (zoom out until the drawing
is about 190 px wide). Iterate at least three rounds.

## Shared rules (both styles)

**Reference first.** Download a photo of the exact model (Wikimedia Commons, Evan Amos's product
shots are ideal) into `.art-preview/ref/` and study it: proportions (measure W x H x D in mm), the
two or three details that make it instantly recognisable, and the real colours.

**View.** Front-on oblique, the same for every console:
- The front face is drawn flat, facing you: its edges are horizontal and vertical, so ports, slots
  and buttons stay in straight rows and read clearly.
- Depth recedes **up and to the right**. You see the front, the top and the **right** side.
- Depth is drawn at about half its real length (details per style below).
- Light comes from the upper left: the top is the lightest face, the front is middle, the right side
  is in shade.
- Draw the console the way people picture it: PS2 horizontal, Wii standing upright, Switch as the
  tablet with Joy-Con attached. If the box itself is anonymous, the thing people remember is the
  identity (for Wii U that is the GamePad).
- Handhelds stand upright and face you straight on, with their thickness shown by the same depth
  offset. Clamshells (DS, 3DS) are drawn open: the lower half lies flat (you see its top surface,
  receding up-right) and the screen half stands up behind it, facing you.
- No controllers, cables, discs or boxes unless they are part of the identity and fit cleanly.
  Consistency across the set beats extra props.

**Placement.** Everything sits on one ground line and is centred horizontally.
- Pixel: the lowest pixels (base or feet) are on **row 29**; rows 30-31 stay empty. The bounding
  box is centred on columns 23-24.
- Ink: the front bottom edge (or the feet) touches **y = 138**. The drawing, including its depth
  and handle, is centred on x = 120.

**Size.** Similar visual weight across the set, true proportions within each console.
- Home consoles: 70-80% of the width (pixel: 36-42 columns; ink: 168-192 units), unless that
  makes it taller than the space (pixel: top no higher than row 3; ink: no higher than y = 12),
  in which case height decides. A flat console (PS2) is simply flatter; do not stretch it.
- Handhelds: pixel 20-26 rows tall; ink about 100-115 units tall.

**Honest detail.** Only the details that identify the console: lid shapes, slot positions, port
counts, vents, the logo's shape. Get those right (count the ports, check which side the vents are
on). Leave out screws, labels, regulatory marks and small text. No wordmarks; a logo mark may be
suggested as a shape (the PS2 top logo is drawn as its three angular strokes).

## Pixel style (48 x 32, 1-bit)

The app draws `#` in the foreground colour: white on black normally, black on white when a tile is
focused. Think of `#` as ink. Crisp, bold, sprite-like: no dithering, no noise, no anti-aliasing.

**Projection.** Front face is an axis-aligned rectangle. Every receding edge is a clean 45 degree
pixel diagonal: one pixel up for each pixel right. The depth offset `d` (equal across and up) is
about one third of the real depth measured at the front's pixel scale:
`d ≈ 0.35 x depthMm x (frontWidthPx / widthMm)`. GameCube: front 26 x 17, d = 9. PS2: front 34 x 12,
d = 8. Round to taste, but keep 45 degrees.

**Dark vs light plastic** (decide by the main body colour):
- **Dark or saturated plastic** (black, charcoal, indigo, dark grey): solid faces. Separate the
  faces with **1 px off seams**: the row just above the front face, the column just right of it,
  and the diagonal between top and side (see `box()` below). Details are cut out as off pixels:
  grooves, vents, light panels, lids. Lighter parts (a grey port panel) become off areas with their
  own dark details drawn back in (the GameCube's four ports are solid dots inside an off panel).
- **Light plastic** (white, light grey): **1 px outline** on the silhouette and on the three visible
  face edges, faces left off (see `obox()`). Give it weight with its dark parts drawn solid: buttons,
  slot shadows, vents, screens, the darker half of a two-tone body.
- Two-tone bodies (NES, SNES, PS5) mix the two: each part follows its own colour.
- Darker-than-body details on a solid body (the GameCube's black disc lid) are separated by a 1 px
  off ring, not by changing their fill.

**Lines and shapes.**
- Lines are 1 px. Allowed slopes: horizontal, vertical, 1:1, 2:1, 1:2. No jaggies, no doubled
  corners (an L of three pixels where two would do).
- Circles and ellipses use clean stepped templates, symmetric left to right (or point-symmetric).
  A lid on a top face is a flat ellipse about 15-17 wide and 5-6 tall; do not shear it, it reads
  better level. Template (17 x 6) used for the GameCube lid:

  ```
  ......#######......
  ...###.......###...
  .##.............##.
  .##.............##.
  ...###.......###...
  ......#######......
  ```
- Small round things (ports, buttons): 4 x 4 with the corners off (`.##.` / `####` / `####` / `.##.`),
  or 2 x 2, or 1 px for LEDs.
- Repeating structure (fins, slats, vents) at **2 px pitch only on horizontal or vertical runs**.
  On the receding side face, keep vents vertical (vertical lines survive the 45 degree shear) and
  never draw diagonal stripes at 2 px pitch: they turn into a checkerboard, which is dithering.
  If a side has fins (PS2), leave the side solid and let the front carry the fins.
- No isolated single pixels except deliberate ones (an LED, a button). No 2 x 2 checker patterns
  anywhere.
- Small feet: leave the bottom row off except two or three pixels under each corner. It lifts the
  console off the ground (GameCube).
- Overhangs: show them (the PS2's ridged slab overhangs its recessed base on the front and the side).
- Logo shapes: only if they read at 1 px strokes (the PS2 logo is 12 x 5, slanted by one pixel every
  two rows so it lies on the top). Otherwise leave the face clean.

## Ink style (paper theme, viewBox 0 0 240 160)

A field-guide / instruction-manual illustration: confident ink outlines, flat muted colour, a little
parallel-line shading, a short hatched shadow. It sits on the paper panel (`var(--panel)`, #f6f4ee)
with ink #141311.

**Projection.** Work in millimetres from the real console: x to the right (width), y up (height),
z to the back (depth), origin at the front-bottom-left corner. Then

```
screenX = ox + (x + 0.38 * z) * k
screenY = 138 - (y + 0.32 * z) * k
```

`k` is the scale (screen units per mm) chosen so the drawing fills 70-80% of the width (GameCube
k = 0.76, PS2 k = 0.52), `ox` centres it. Depth recedes at about 40 degrees, at half length. Circles
on the front are true circles; circles on the top or side are ellipses (use `ell()` below).

**Line weights** (in viewBox units; the tile draws at about 0.8x):

| What | stroke-width |
| --- | --- |
| Outer silhouette, drawn last over everything | 2.4 |
| Edges between faces (top/front, front/side, top/side) | 1.9 |
| Parts: panels, lids, doors, trays, port rims | 1.3 - 1.8 |
| Small details: seams, slots, buttons, grille borders | 1.0 - 1.4 |
| Hatching and shading lines | 0.9 - 1.0 |
| Ground line | 1.2 |

All lines `currentColor`, round caps and joins (set once on the root `<g>`), never thinner than 0.8.
Dots (port markers, LEDs) are `<circle r='0.7'..'0.9'>` filled, no stroke.

**Fills.**
- One body colour, muted: low saturation (HSL saturation under about 25%), middle lightness
  (about 35-70%) so ink lines stay visible on it. Use it in up to three values for the three faces:
  top lightest, front middle, side darkest (GameCube: `#a9a5c2`, `#8a86a9`, `#7e7a9d`; PS2:
  `#6d6a65`, `#57554f`, `#4a4844`).
- White or light-grey plastic is `var(--panel)`, shaded only with hatching.
- One accent family at most, also muted (the PS2's blue I/O panel `#7487ab` and logo `#9fb0d0`).
- Darks for holes, discs, rubber, vents and handles: `#2a2926` to `#4b4858`. Never pure black: the
  ink outline must still show around it.
- No gradients. Glossy things (a disc window) can take one short highlight stroke in `var(--panel)`
  at opacity 0.5-0.6.

**Shading.**
- The right side face is in shade: vertical hatch lines, 0.9 wide, about 4.2-4.5 apart, inset 1.5
  from the face edges, opacity 0.55-0.6 over a coloured fill (1.0 over `var(--panel)`).
- On a dark body, hatching disappears: use the darker side value instead and add highlights.
- Ridges, fins and grooves: an ink line for the groove plus a thin `var(--panel)` line (0.8 wide,
  opacity 0.2-0.35) just below it, where the next fin catches the light.
- Shadow under an overhang: 2-3 short horizontal hatch lines, opacity 0.7.

**Ground.**
- Ground line: a horizontal 1.2 line at y = 138 from about 6-16 mm left of the console to the
  front end of its shadow.
- Hatched shadow: horizontal lines, 1.0 wide, 3.2-3.3 apart, filling the parallelogram from the
  side face's footprint out to the right, about 12-20 mm wide at the front and 24-30 mm at the back.
  Nothing else on the ground.

**Drawing order.** Ground shadow and ground line, things behind the body (a handle that wraps
behind), feet, the faces (top, side, front) with their fills, side hatching, details on each face,
things in front of a face (a strap on the side), then the edge lines and the outer silhouette last.

**Watch out.** A shape that folds back on itself (a strap that runs back and then turns) must be
split into two polygons at the turning point, or the overlap is left unfilled.

## Drafting kit

Copy into a scratch script (keep it out of the repo). It prints the strings you paste into the JSON.

```js
// ---- pixel ----
const grid = () => Array.from({ length: 32 }, () => Array(48).fill(0));
const set = (g, x, y, v = 1) => { if (x >= 0 && y >= 0 && x < 48 && y < 32) g[y][x] = v; };
const hline = (g, x0, x1, y, v = 1) => { for (let x = x0; x <= x1; x++) set(g, x, y, v); };
const vline = (g, x, y0, y1, v = 1) => { for (let y = y0; y <= y1; y++) set(g, x, y, v); };
const rows = (g) => g.map((r) => r.map((v) => (v ? "#" : ".")).join(""));
// solid box with off seams (dark plastic). Front: columns a..a+w-1, rows t..t+h-1. Depth d at 45 degrees.
function box(g, a, t, w, h, d) {
  for (let y = t; y < t + h; y++) hline(g, a, a + w - 1, y);
  for (let k = 1; k <= d; k++) { hline(g, a + k, a + w - 1 + k, t - k); vline(g, a + w - 1 + k, t - k, t + h - 1 - k); }
  hline(g, a + 1, a + w, t - 1, 0); vline(g, a + w, t - 1, t + h - 2, 0);
  for (let k = 2; k <= d; k++) set(g, a + w - 1 + k, t - k, 0);
}
// outlined box (light plastic)
function obox(g, a, t, w, h, d) {
  const R = a + w - 1, B = t + h - 1;
  hline(g, a, R, t); hline(g, a, R, B); vline(g, a, t, B); vline(g, R, t, B);
  for (let k = 1; k <= d; k++) { set(g, a + k, t - k); set(g, R + k, t - k); set(g, R + k, B - k); }
  hline(g, a + d, R + d, t - d); vline(g, R + d, t - d, B - d);
}

// ---- ink ----
const r1 = (n) => Math.round(n * 10) / 10;
const ink = ({ ox, k, cx = 0.38, cy = 0.32 }) => {
  const P = (x, y, z = 0) => [ox + (x + z * cx) * k, 138 - (y + z * cy) * k];
  const path = (pts, close = true) => "M" + pts.map((p) => r1(p[0]) + " " + r1(p[1])).join("L") + (close ? "Z" : "");
  // circle of radius r lying on a plane: 'front' (z = fixed), 'top' (y = fixed), 'side' (x = fixed)
  const ell = (plane, c1, c2, fixed, r, attrs = "") => {
    const o = plane === "top" ? P(c1, fixed, c2) : plane === "front" ? P(c1, c2, fixed) : P(fixed, c2, c1);
    const a = plane === "side" ? [cx * k, -cy * k] : [k, 0], b = plane === "top" ? [cx * k, -cy * k] : [0, -k];
    const [m11, m21, m12, m22] = [a[0] * r, a[1] * r, b[0] * r, b[1] * r];
    const E = (m11 + m22) / 2, F = (m11 - m22) / 2, G = (m21 + m12) / 2, H = (m21 - m12) / 2;
    const Q = Math.hypot(E, H), R = Math.hypot(F, G), th = ((Math.atan2(G, F) + Math.atan2(H, E)) / 2) * 180 / Math.PI;
    return `<ellipse cx='${r1(o[0])}' cy='${r1(o[1])}' rx='${r1(Q + R)}' ry='${r1(Math.abs(Q - R))}' transform='rotate(${r1(th)} ${r1(o[0])} ${r1(o[1])})' ${attrs}/>`;
  };
  return { P, path, ell };
};
// parallel hatch lines inside a convex polygon of screen points. angle: 0 = horizontal, 90 = vertical.
function hatch(poly, angle, gap, attrs = "", inset = 0) {
  const t = (angle * Math.PI) / 180, dx = Math.cos(t), dy = Math.sin(t), nx = -dy, ny = dx;
  const n = poly.map(([x, y]) => x * nx + y * ny); let d = "";
  for (let c = Math.min(...n) + gap / 2; c < Math.max(...n); c += gap) {
    const hits = [];
    poly.forEach(([ax, ay], i) => {
      const [bx, by] = poly[(i + 1) % poly.length], pa = ax * nx + ay * ny - c, pb = bx * nx + by * ny - c;
      if ((pa <= 0) !== (pb <= 0)) { const u = pa / (pa - pb); hits.push([ax + (bx - ax) * u, ay + (by - ay) * u]); }
    });
    if (hits.length < 2) continue;
    hits.sort((p, q) => p[0] * dx + p[1] * dy - (q[0] * dx + q[1] * dy));
    const [p, q] = [hits[0], hits[hits.length - 1]];
    if (Math.hypot(q[0] - p[0], q[1] - p[1]) < inset * 2 + 0.5) continue;
    d += `M${r1(p[0] + dx * inset)} ${r1(p[1] + dy * inset)}L${r1(q[0] - dx * inset)} ${r1(q[1] - dy * inset)}`;
  }
  return d ? `<path d='${d}' ${attrs}/>` : "";
}
```

## What the two reference drawings show

- **ps2** (original fat PS2, horizontal). Pixel: solid slab with four 1 px fins across the front,
  tray seams, the two-button column at the right end, the slab overhanging a recessed base with the
  I/O panel and slatted vent, side left solid, the PS2 logo shape slanted on the top. Ink: charcoal
  in three values, five fins with groove line plus highlight, fins continuing along the side,
  notches in the silhouette where the grooves meet the edges, staggered memory-card slots and
  controller ports, blue I/O panel, grid vent, the rotated PS2 logo lying on the top as on the real
  console, overhang shadow on the base.
- **gc** (GameCube, indigo). Pixel: solid cube with off seams, off port panel with four round
  ports and two memory-card doors, level ellipse lid ring, vertical vent slots on the side, handle
  loop at the back, feet. Ink: indigo in three values, `var(--panel)` port panel, ports with rims
  and their 1-4 dot markers, memory-card doors, lid plate, dark disc window with one highlight,
  power/reset/open buttons, side vent grid with its lip, hatched side, the black strap handle
  sticking out behind the right side (split at its turning point), feet.

## Checklist

- [ ] Reference photo of the exact model studied; proportions measured.
- [ ] JSON valid; 32 rows of 48; only `#` and `.`; ink uses single quotes and only allowed elements.
- [ ] Front-on oblique, depth up-right, right side visible, light from the upper left.
- [ ] Pixel: bottom on row 29, centred, 45 degree receding edges, dark = solid with seams,
      light = outlined, no dithering, no checkerboard, no stray pixels.
- [ ] Ink: ground line at y = 138, centred, 70-80% width, stroke weights per table, one muted body
      colour in up to three values, `var(--panel)` for light plastic, side hatched, short hatched
      ground shadow, outline drawn last, no text.
- [ ] Recognisable at tile size (about 190 px wide) in both styles, including the inverted pixel
      version (black on white).
- [ ] Looked at on the contact sheet next to ps2 and gc: same angle, same ground, similar weight.
