#!/usr/bin/env node
// Builds crates/core/data/ports.json from:
//   catalog/reference-catalog.json  the research catalog (351 entries, from the Notion project)
//   catalog/ports.curated.json      hand-written entries/overrides (exe names, saves to keep, art)
// Only ports whose GitHub releases actually contain a Windows archive are listed, so every
// "Get latest" button in the app works. Run: node scripts/build-ports.mjs  (uses GITHUB_TOKEN or `gh auth token`)

import { readFileSync, writeFileSync } from "node:fs";
import { execSync } from "node:child_process";

const root = new URL("..", import.meta.url);
const read = (p) => JSON.parse(readFileSync(new URL(p, root), "utf8"));
const reference = read("catalog/reference-catalog.json").entries;
const curated = read("catalog/ports.curated.json");

let token = process.env.GITHUB_TOKEN;
if (!token) {
  try {
    token = execSync("gh auth token", { encoding: "utf8" }).trim();
  } catch {
    console.warn("No GITHUB_TOKEN and no gh CLI: limited to 60 requests/hour.");
  }
}

// Same rules as windows_score() in crates/core/src/github.rs.
const BAD = ["arm64", "aarch64", "linux", "macos", "mac-", "-mac", "osx", "darwin", "android", "ios", "source", "-src", "debug",
  "pdb", "symbols", "dbg", "appimage", "flatpak", "win32", "i686", "x86-", "-x86.", "libretro"];
function windowsScore(name) {
  const n = name.toLowerCase();
  if (!(n.endsWith(".zip") || n.endsWith(".7z"))) return null;
  if (BAD.some((b) => n.includes(b))) return null;
  if (!["win", "msvc", "mingw", "w64"].some((k) => n.includes(k))) return null;
  let s = 10;
  if (["x64", "x86_64", "win64", "amd64"].some((k) => n.includes(k))) s += 3;
  if (n.includes("msvc")) s += 1;
  if (n.includes("portable")) s += 1;
  if (n.includes("installer") || n.includes("setup")) s -= 4;
  return s;
}

async function releases(repo) {
  const res = await fetch(`https://api.github.com/repos/${repo}/releases?per_page=8`, {
    headers: { Accept: "application/vnd.github+json", ...(token ? { Authorization: `Bearer ${token}` } : {}) },
  });
  if (!res.ok) return null;
  return res.json();
}

const KIND = (t) =>
  t.startsWith("Recomp") ? "Recomp" : t.startsWith("Decomp") ? "Decomp" : t.startsWith("Engine") ? "Engine recreation" : "Native port";

const candidates = reference.filter(
  (e) =>
    /^Yes/.test(e.windows) &&
    /github\.com\/[^/]+\/[^/#?]+/.test(e.url) &&
    !/Emulator|Frontend|Metadata|Dev tool/.test(e.type),
);

const out = new Map(curated.map((c) => [c.id, c]));
const skipped = [];
let i = 0;
async function worker() {
  while (i < candidates.length) {
    const e = candidates[i++];
    if (out.has(e.id)) continue;
    const repo = e.url.match(/github\.com\/([^/]+\/[^/#?]+)/)[1].replace(/\.git$/, "");
    const rels = await releases(repo);
    if (!rels?.length) {
      skipped.push(`${e.id}: no releases`);
      continue;
    }
    const stable = rels.find((r) => !r.prerelease && r.assets.some((a) => windowsScore(a.name) !== null));
    const any = rels.find((r) => r.assets.some((a) => windowsScore(a.name) !== null));
    const rel = stable ?? any;
    if (!rel) {
      skipped.push(`${e.id}: no Windows archive in ${repo}`);
      continue;
    }
    const game = (e.notes?.match(/Game: (.+?)\. Runs on/)?.[1] ?? e.notes?.match(/Game: (.+?)\.?$/)?.[1] ?? e.name).trim();
    const req = e.required_files?.replace(/^user's own/, "Your own").replace(/ \(.*\)$/, "");
    out.set(e.id, {
      id: e.id,
      game,
      name: e.name,
      kind: KIND(e.type),
      original: e.consoles ?? [],
      repo,
      asset: "auto",
      prerelease: !stable,
      requires: req ? `${req[0].toUpperCase()}${req.slice(1)}.` : null,
      licence: e.licence && e.licence !== "unknown" ? e.licence : null,
      blurb: e.status ? `${e.maintainer ? `By ${e.maintainer}. ` : ""}${e.status[0].toUpperCase()}${e.status.slice(1)}.` : null,
      featured: false,
    });
  }
}
await Promise.all(Array.from({ length: 8 }, worker));

const ports = [...out.values()].sort((a, b) => Number(!!b.featured) - Number(!!a.featured) || a.game.localeCompare(b.game));
writeFileSync(new URL("crates/core/data/ports.json", root), JSON.stringify(ports, null, 2) + "\n");
console.log(`${ports.length} installable ports written (${skipped.length} skipped).`);
if (process.argv.includes("--verbose")) console.log(skipped.join("\n"));
