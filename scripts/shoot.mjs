#!/usr/bin/env node
// Screenshot the UI the way it ships: headless Edge (the same engine as the app's WebView2)
// at real window sizes, both themes, every screen. Needs the dev server on :1420 with
// VITE_ONIONRING_DATA set (see .env.example) so the UI runs against a real library snapshot.
//
//   node scripts/shoot.mjs                       all themes, sizes, views
//   node scripts/shoot.mjs --views home,tidy --sizes 1440x900 --themes paper
//   node scripts/shoot.mjs --out .shots/round2
//
// Writes <out>/<theme>-<w>x<h>-<view>.png and prints the file list.

import { chromium } from "playwright-core";
import { mkdirSync } from "node:fs";
import { join } from "node:path";

const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : fallback;
};

// The app's default window, a maximised 1080p and 1440p screen at 125% scaling, and the minimum.
const SIZES = arg("sizes", "1440x900,1536x864,2048x1152,1024x640").split(",").map((s) => s.split("x").map(Number));
const THEMES = arg("themes", "pixel,paper").split(",");
const OUT = arg("out", ".shots");
const URL = arg("url", "http://127.0.0.1:1420/");
const MOTION = arg("motion", "reduce"); // "reduce" = settled frames; "no-preference" = mid-animation

/** Each view: how to get there from a fresh load (runs in the page). */
const VIEWS = {
  home: `app.go({ name: "home" })`,
  "home-scrolled": `app.go({ name: "home" }); await wait(500); document.querySelector('.home, main [class*="home"]')?.scrollTo(0, 99999)`,
  library: `app.go({ name: "library" })`,
  game: `app.openGame(app.games.find((g) => g.title === "God of War III") ?? app.games[0])`,
  "game-port": `app.openGame(app.games.find((g) => g.kind === "port") ?? app.games[0])`,
  "get-ports": `app.go({ name: "get", tab: "ports" })`,
  "get-port-detail": `app.go({ name: "get", tab: "ports", item: "2-ship-2-harkinian-the-legend-of-zelda-majora-s-mask" })`,
  "get-emulators": `app.go({ name: "get", tab: "emulators" })`,
  "get-browser": `app.go({ name: "get", tab: "browser" })`,
  "get-mashups": `app.go({ name: "get", tab: "mashups" })`,
  "get-mashup-detail": `app.go({ name: "get", tab: "mashups", item: "skycraft" })`,
  "get-mashup-missing": `app.go({ name: "get", tab: "mashups", item: "eldencraft" })`,
  "get-updates": `app.go({ name: "get", tab: "updates" })`,
  systems: `app.go({ name: "systems" })`,
  tidy: `app.go({ name: "tidy" })`,
  settings: `app.go({ name: "settings" })`,
  search: `app.query = "zelda"; app.searchOpen = true`,
  menu: `app.menuOpen = true`,
};
// Views added by later features can be shot by name if the app exposes them via app.go().
const extra = (name) => `app.go({ name: ${JSON.stringify(name)} })`;
const views = arg("views", Object.keys(VIEWS).join(",")).split(",");

mkdirSync(OUT, { recursive: true });
const browser = await chromium.launch({ channel: "msedge", headless: true });
const files = [];
for (const theme of THEMES) {
  for (const [w, h] of SIZES) {
    const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: 1, reducedMotion: MOTION });
    await ctx.addInitScript((t) => localStorage.setItem("onionring.theme", JSON.stringify(t)), theme);
    const page = await ctx.newPage();
    const errors = [];
    page.on("pageerror", (e) => errors.push(String(e)));
    page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
    for (const v of views) {
      await page.goto(URL, { waitUntil: "networkidle" });
      await page.waitForFunction(() => window.onionring?.loaded, null, { timeout: 15000 });
      const code = VIEWS[v] ?? extra(v);
      await page.evaluate(`(async () => { const app = window.onionring; const wait = (ms) => new Promise((r) => setTimeout(r, ms)); ${code}; })()`);
      await page.waitForTimeout(1400);
      const file = join(OUT, `${theme}-${w}x${h}-${v}.png`);
      await page.screenshot({ path: file });
      files.push(file);
    }
    if (errors.length) console.error(`[${theme} ${w}x${h}] page errors:\n  ${[...new Set(errors)].join("\n  ")}`);
    await ctx.close();
  }
}
await browser.close();
console.log(files.join("\n"));
