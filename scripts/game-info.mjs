#!/usr/bin/env node
// Looks up each port's game once and stamps what it finds into crates/core/data/ports.json:
//   about     summary from Wikipedia; release date, developer, publisher, genres, series from
//             Wikidata (no key needed)
//   pictures  cover (600x900), wide grids, hero and logo from SteamGridDB (needs SGDB_API_KEY;
//             skipped without it)
// Results are kept in catalog/game-info.json, keyed by game, so the weekly refresh only asks
// about new games (and retries misses after a week). The app never needs a key: it reads the
// URLs and facts from the catalog.
//
// A wrong match is fixed by hand in catalog/game-info.json: set "wikidataId" (e.g. "Q1129045")
// and/or "sgdbId" on the entry and run again. Run: node scripts/game-info.mjs
// --force looks everything up again.

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = new URL("..", import.meta.url);
const CACHE = new URL("catalog/game-info.json", root);
const PORTS = new URL("crates/core/data/ports.json", root);
const DAY = 24 * 3600 * 1000;
const FOUND_TTL = 90 * DAY;
const MISS_TTL = 7 * DAY;
// Wikimedia asks every client to say who it is.
const UA = { "User-Agent": "OnionRing-catalog/1.0 (https://github.com/George-Nizor/OnionRing)" };

export const gameKey = (s) =>
  s.normalize("NFKD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/&/g, "and").replace(/[^a-z0-9]+/g, " ").trim();

export function loadCache() {
  return existsSync(CACHE) ? JSON.parse(readFileSync(CACHE, "utf8")) : {};
}

/** Put each game's pictures and facts on its ports, replacing what an earlier run put there. */
export function applyInfo(ports, cache = loadCache()) {
  for (const p of ports) {
    delete p.pictures;
    delete p.about;
    const e = cache[gameKey(p.game)];
    if (e?.pictures && Object.keys(e.pictures).length) p.pictures = e.pictures;
    if (e?.about) p.about = e.about;
  }
  return ports;
}

// Bigram overlap (Dice), for names that differ by punctuation or a subtitle.
function similar(a, b) {
  const grams = (s) => {
    const out = new Map();
    for (let i = 0; i < s.length - 1; i++) out.set(s.slice(i, i + 2), (out.get(s.slice(i, i + 2)) ?? 0) + 1);
    return out;
  };
  const [x, y] = [grams(a.replace(/ /g, "")), grams(b.replace(/ /g, ""))];
  let hit = 0;
  for (const [g, n] of x) hit += Math.min(n, y.get(g) ?? 0);
  const total = [...x.values(), ...y.values()].reduce((s, n) => s + n, 0);
  return total ? (2 * hit) / total : 0;
}

async function getJson(url, headers = UA) {
  for (let tries = 0; ; tries++) {
    const r = await fetch(url, { headers });
    if (r.status === 429 && tries < 3) {
      await new Promise((ok) => setTimeout(ok, 2000 * (tries + 1)));
      continue;
    }
    if (r.status === 404) return null;
    if (!r.ok) throw new Error(`${r.status} for ${url}`);
    return r.json();
  }
}

// ---- Wikidata + Wikipedia ---------------------------------------------------------------------

const WD = "https://www.wikidata.org/w/api.php?format=json&";
const WP = "https://en.wikipedia.org/w/api.php?format=json&";

// Our console names as Wikidata labels them, where they differ.
const PLATFORM_NAMES = {
  SNES: ["Super Nintendo Entertainment System"],
  NES: ["Nintendo Entertainment System", "Family Computer"],
  GameCube: ["Nintendo GameCube", "GameCube"],
  PSP: ["PlayStation Portable"],
  "Sega Genesis": ["Sega Genesis", "Mega Drive"],
  "Sega 32X": ["32X", "Sega 32X"],
  "PC (original release)": ["Microsoft Windows", "MS-DOS", "DOS", "macOS", "classic Mac OS", "Linux"],
  Mobile: ["iOS", "Android"],
  Arcade: ["arcade video game", "arcade game machine", "arcade cabinet"],
};

const labels = new Map(); // Q-id -> English label
// Many items keep their name under "mul" (the same in every language) rather than "en".
const label = (e) => e?.labels?.en?.value ?? e?.labels?.mul?.value ?? null;
async function entities(ids, props) {
  const out = {};
  for (let i = 0; i < ids.length; i += 50) {
    const j = await getJson(`${WD}action=wbgetentities&ids=${ids.slice(i, i + 50).join("|")}&props=${props}&languages=en|mul&sitefilter=enwiki`);
    Object.assign(out, j?.entities ?? {});
  }
  return out;
}
async function nameAll(ids) {
  const missing = [...new Set(ids)].filter((id) => !labels.has(id));
  if (missing.length) {
    const got = await entities(missing, "labels");
    for (const id of missing) labels.set(id, label(got[id]));
  }
}

const values = (item, prop) =>
  (item.claims?.[prop] ?? []).filter((c) => c.rank !== "deprecated").map((c) => c.mainsnak?.datavalue?.value).filter(Boolean);
const refs = (item, prop) => values(item, prop).map((v) => v.id).filter(Boolean);
const named = (item, prop) => [...new Set(refs(item, prop).map((id) => labels.get(id)).filter(Boolean))];

// Sequel numbers, Arabic or Roman: "Civilization III" is not "Civilization II".
const ROMAN = { ii: 2, iii: 3, iv: 4, v: 5, vi: 6, vii: 7, viii: 8, ix: 9, x: 10 };
const numbers = (key) => key.split(" ").map((w) => (/^d+$/.test(w) ? Number(w) : ROMAN[w])).filter((n) => n !== undefined).join(",");

/** Earliest publication date, as precise as Wikidata knows it: YYYY, YYYY-MM or YYYY-MM-DD. */
function released(item) {
  const dates = values(item, "P577")
    .map((v) => ({ t: v.time?.replace(/^\+/, ""), p: v.precision }))
    .filter((d) => d.t && d.p >= 9)
    .sort((a, b) => a.t.localeCompare(b.t));
  const d = dates[0];
  return d ? d.t.slice(0, d.p >= 11 ? 10 : d.p === 10 ? 7 : 4) : null;
}

async function candidates(game) {
  const ids = new Set();
  const wd = await getJson(`${WD}action=wbsearchentities&search=${encodeURIComponent(game)}&language=en&type=item&limit=10`);
  (wd?.search ?? []).forEach((s) => ids.add(s.id));
  const wp = await getJson(
    `${WP}action=query&generator=search&gsrsearch=${encodeURIComponent(`${game} video game`)}&gsrlimit=6&prop=pageprops&ppprop=wikibase_item`,
  );
  Object.values(wp?.query?.pages ?? {}).forEach((p) => p.pageprops?.wikibase_item && ids.add(p.pageprops.wikibase_item));
  return [...ids];
}

export async function wikiLookup(game, consoles, pinned) {
  const ids = pinned ? [pinned] : await candidates(game);
  if (!ids.length) return null;
  const items = Object.values(await entities(ids, "labels|aliases|claims|sitelinks")).filter((it) => it.claims);
  // Video games: an instance of "video game", or anything with a platform, but not a series.
  const isGame = (it) => (refs(it, "P31").includes("Q7889") || refs(it, "P400").length) && !refs(it, "P31").includes("Q7058673");
  const games = pinned ? items : items.filter(isGame);
  await nameAll(games.flatMap((it) => ["P400", "P178", "P123", "P136", "P179"].flatMap((p) => refs(it, p))));

  const want = gameKey(game);
  const plats = new Set(consoles.flatMap((c) => PLATFORM_NAMES[c] ?? [c]).map((n) => n.toLowerCase()));
  const scored = games
    .map((it) => {
      const article = it.sitelinks?.enwiki?.title?.replace(/ (.*)$/, "");
      const names = [label(it), article, ...[...(it.aliases?.en ?? []), ...(it.aliases?.mul ?? [])].map((a) => a.value)].filter(Boolean).map(gameKey);
      const close = names.filter((n) => numbers(n) === numbers(want));
      const sim = names.includes(want) ? 1 : Math.max(0, ...close.map((n) => similar(n, want)));
      const onPlatform = named(it, "P400").some((n) => plats.has(n.toLowerCase()));
      const hasArticle = !!it.sitelinks?.enwiki;
      return { it, sim, onPlatform, score: sim * 10 + (onPlatform ? 3 : 0) + (hasArticle ? 1 : 0) };
    })
    .sort((a, b) => b.score - a.score);
  // A close name on the right console, else the exact name on any.
  const best = pinned ? scored[0] : (scored.find((s) => s.sim >= 0.85 && s.onPlatform) ?? scored.find((s) => s.sim === 1));
  if (!best) return null;

  const it = best.it;
  const title = it.sitelinks?.enwiki?.title;
  let summary = null;
  if (title) {
    const s = await getJson(`https://en.wikipedia.org/api/rest_v1/page/summary/${encodeURIComponent(title.replace(/ /g, "_"))}`);
    if (s?.type === "standard") summary = s.extract || null;
  }
  const developer = named(it, "P178");
  const publisher = named(it, "P123");
  return {
    id: it.id,
    about: {
      summary,
      released: released(it),
      developer: developer.join(", ") || null,
      publisher: publisher.join(", ") || null,
      genres: named(it, "P136").map((g) => g[0].toUpperCase() + g.slice(1)),
      series: named(it, "P179")[0] ?? null,
      wikipedia: title ? `https://en.wikipedia.org/wiki/${encodeURIComponent(title.replace(/ /g, "_"))}` : null,
    },
  };
}

// ---- SteamGridDB ------------------------------------------------------------------------------

async function sgdb(path) {
  const key = process.env.SGDB_API_KEY;
  if (!key) return null;
  return (await getJson(`https://www.steamgriddb.com/api/v2${path}`, { Authorization: `Bearer ${key}` }))?.data ?? [];
}

const CLEAN = "types=static&nsfw=false&humor=false&epilepsy=false";
const best = (list, n = 1) => [...(list ?? [])].sort((a, b) => (b.score ?? 0) - (a.score ?? 0)).slice(0, n).map((x) => x.url);

async function sgdbLookup(game, year, pinned) {
  let id = pinned;
  if (!id) {
    const hits = await sgdb(`/search/autocomplete/${encodeURIComponent(game)}`);
    if (!hits) return undefined; // no key
    const want = gameKey(game);
    const exact = hits.filter((h) => gameKey(h.name) === want);
    // Several games share a name ("Doom"): the one released closest to Wikidata's date.
    const yearOf = (h) => (h.release_date ? new Date(h.release_date * 1000).getUTCFullYear() : null);
    const pick = year && exact.length > 1 ? [...exact].sort((a, b) => Math.abs((yearOf(a) ?? 0) - year) - Math.abs((yearOf(b) ?? 0) - year))[0] : exact[0];
    id = pick?.id ?? hits.find((h) => similar(gameKey(h.name), want) >= 0.9)?.id;
    if (!id) return null;
  }
  const [cover, grids, heroes, logos] = await Promise.all([
    sgdb(`/grids/game/${id}?dimensions=600x900&${CLEAN}`),
    sgdb(`/grids/game/${id}?dimensions=920x430,460x215&${CLEAN}`),
    sgdb(`/heroes/game/${id}?${CLEAN}`),
    sgdb(`/logos/game/${id}?${CLEAN}`),
  ]);
  const pictures = {};
  const set = (k, v) => v != null && (Array.isArray(v) ? v.length : true) && (pictures[k] = v);
  set("cover", best(cover)[0]);
  set("grids", best(grids, 3));
  set("hero", best(heroes)[0]);
  set("logo", best(logos)[0]);
  return { id, pictures };
}

// ---- run --------------------------------------------------------------------------------------

async function main() {
  const force = process.argv.includes("--force");
  const ports = JSON.parse(readFileSync(PORTS, "utf8"));
  const cache = loadCache();
  const games = new Map();
  for (const p of ports) {
    const k = gameKey(p.game);
    const g = games.get(k) ?? { game: p.game, consoles: new Set() };
    p.original.forEach((c) => g.consoles.add(c));
    games.set(k, g);
  }
  if (!process.env.SGDB_API_KEY) console.warn("No SGDB_API_KEY: pictures skipped.");

  const now = Date.now();
  let looked = 0;
  const save = () => {
    const sorted = Object.fromEntries(Object.entries(cache).sort(([a], [b]) => a.localeCompare(b)));
    writeFileSync(CACHE, JSON.stringify(sorted, null, 2) + "\n");
    return sorted;
  };
  for (const [k, { game, consoles }] of games) {
    const e = (cache[k] ??= { game });
    const due = (field, ttl) => force || !e[field] || now - Date.parse(e[field]) > ttl;
    try {
      if (due("aboutChecked", e.about ? FOUND_TTL : MISS_TTL)) {
        const r = await wikiLookup(game, [...consoles], e.wikidataId);
        e.wikidataId = r?.id ?? e.wikidataId;
        if (r) e.about = r.about;
        e.aboutChecked = new Date(now).toISOString();
        looked++;
      }
      if (due("picturesChecked", e.pictures ? FOUND_TTL : MISS_TTL)) {
        const year = e.about?.released ? Number(e.about.released.slice(0, 4)) : null;
        const r = await sgdbLookup(game, year, e.sgdbId);
        if (r !== undefined) {
          e.sgdbId = r?.id ?? e.sgdbId;
          if (r) e.pictures = r.pictures;
          e.picturesChecked = new Date(now).toISOString();
          looked++;
        }
      }
    } catch (err) {
      console.warn(`${game}: ${err.message}`);
    }
    if (looked && looked % 25 === 0) save();
  }

  const sorted = save();
  applyInfo(ports, sorted);
  writeFileSync(PORTS, JSON.stringify(ports, null, 2) + "\n");
  const n = (f) => ports.filter((p) => p[f]).length;
  const missed = Object.values(sorted).filter((e) => !e.about).map((e) => e.game);
  console.log(`${looked} lookups. ${n("about")} of ${ports.length} ports have game info, ${n("pictures")} have pictures.`);
  if (process.argv.includes("--verbose") && missed.length) console.log(`No game info: ${missed.join(" | ")}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) await main();
