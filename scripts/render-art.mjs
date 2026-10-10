#!/usr/bin/env node
// Render console art (src/lib/art/systems/<id>.json) to PNGs you can look at.
//
//   node scripts/render-art.mjs ps2 gc        -> .art-preview/ps2-pixel.png, ps2-ink.png, ...
//   node scripts/render-art.mjs --sheet       -> .art-preview/sheet-pixel.png, sheet-ink.png (every console)
//
// Art format (see src/lib/art/systems/README.md):
//   { "pixel": ["....##....", ...],   48 columns x 32 rows, '#' on, '.' off
//     "ink": "<g>...</g>" }           SVG markup for viewBox 0 0 240 160
// The ink art may use currentColor and the theme variables var(--accent), var(--accent-2),
// var(--accent-3), var(--panel), var(--fg); they are swapped for the paper theme's values here.

import { Resvg } from "@resvg/resvg-js";
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const DIR = "src/lib/art/systems";
const OUT = ".art-preview";
mkdirSync(OUT, { recursive: true });

const PAPER = { bg: "#eeebe3", panel: "#f6f4ee", fg: "#141311", accent: "#b5452f", "accent-2": "#2f5d8a", "accent-3": "#c29433" };
const inkColors = (s) =>
  s
    .replaceAll("currentColor", PAPER.fg)
    .replace(/var\(--(accent-2|accent-3|accent|panel|fg|bg)[^)]*\)/g, (_, k) => PAPER[k]);

function pixelRects(rows, x0 = 0, y0 = 0, scale = 1, color = "#fff") {
  let out = "";
  rows.forEach((row, y) => {
    let x = 0;
    while (x < row.length) {
      if (row[x] === "#") {
        let w = 1;
        while (row[x + w] === "#") w++;
        out += `<rect x="${x0 + x * scale}" y="${y0 + y * scale}" width="${w * scale}" height="${scale}" fill="${color}"/>`;
        x += w;
      } else x++;
    }
  });
  return out;
}

function check(id, art) {
  const problems = [];
  if (!Array.isArray(art.pixel)) problems.push("pixel is not an array");
  else {
    if (art.pixel.length !== 32) problems.push(`pixel has ${art.pixel.length} rows (want 32)`);
    art.pixel.forEach((r, i) => r.length !== 48 && problems.push(`pixel row ${i} is ${r.length} wide (want 48)`));
    art.pixel.forEach((r, i) => /[^#.]/.test(r) && problems.push(`pixel row ${i} has characters other than # and .`));
  }
  if (typeof art.ink !== "string" || !art.ink.includes("<")) problems.push("ink is missing");
  if (problems.length) console.error(`${id}: ${problems.join("; ")}`);
  return problems.length === 0;
}

const render = (svg, file, width) => {
  const png = new Resvg(svg, { fitTo: { mode: "width", value: width }, background: undefined }).render().asPng();
  writeFileSync(file, png);
  console.log(file);
};

const load = (id) => JSON.parse(readFileSync(join(DIR, `${id}.json`), "utf8"));
const ids = process.argv.slice(2).filter((a) => !a.startsWith("--"));
const all = readdirSync(DIR).filter((f) => f.endsWith(".json")).map((f) => f.slice(0, -5)).sort();

if (process.argv.includes("--sheet")) {
  const cols = 4;
  const items = all.map((id) => ({ id, art: load(id) })).filter(({ id, art }) => check(id, art));
  const rows = Math.ceil(items.length / cols);
  // pixel sheet: 48x32 cells scaled x6, 24px gutters, labels omitted (filenames list order)
  const S = 6, CW = 48 * S + 40, CH = 32 * S + 40;
  let px = `<svg xmlns="http://www.w3.org/2000/svg" width="${cols * CW}" height="${rows * CH}"><rect width="100%" height="100%" fill="#000"/>`;
  let ink = `<svg xmlns="http://www.w3.org/2000/svg" width="${cols * 280}" height="${rows * 200}"><rect width="100%" height="100%" fill="${PAPER.bg}"/>`;
  items.forEach(({ art }, i) => {
    const cx = i % cols, cy = Math.floor(i / cols);
    px += pixelRects(art.pixel, cx * CW + 20, cy * CH + 20, S);
    ink += `<g transform="translate(${cx * 280 + 20} ${cy * 200 + 20})"><rect width="240" height="160" fill="${PAPER.panel}" stroke="${PAPER.fg}" stroke-width="1"/><svg width="240" height="160" viewBox="0 0 240 160" style="color:${PAPER.fg}">${inkColors(art.ink)}</svg></g>`;
  });
  render(px + "</svg>", join(OUT, "sheet-pixel.png"), cols * CW);
  render(ink + "</svg>", join(OUT, "sheet-ink.png"), cols * 280 * 2);
  console.log(`order: ${items.map((x) => x.id).join(", ")}`);
} else {
  for (const id of ids.length ? ids : all) {
    const art = load(id);
    if (!check(id, art)) continue;
    render(`<svg xmlns="http://www.w3.org/2000/svg" width="${48 * 10}" height="${32 * 10}"><rect width="100%" height="100%" fill="#000"/>${pixelRects(art.pixel, 0, 0, 10)}</svg>`, join(OUT, `${id}-pixel.png`), 480);
    render(
      `<svg xmlns="http://www.w3.org/2000/svg" width="240" height="160" viewBox="0 0 240 160" style="color:${PAPER.fg}"><rect width="240" height="160" fill="${PAPER.panel}"/>${inkColors(art.ink)}</svg>`,
      join(OUT, `${id}-ink.png`),
      960,
    );
  }
}
