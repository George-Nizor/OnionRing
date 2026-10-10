import { Resvg } from "@resvg/resvg-js";
import fs from "node:fs";
const deg = (a) => (a * Math.PI) / 180;
const on = (px, y) => {
  const cx = px + 0.5, cy = y + 0.5;
  const d = Math.hypot(cx - 12, cy - 12);
  const inTri = cx >= 9.5 && cx <= 16.5 && Math.abs(cy - 12) <= (16.5 - cx) * 0.62;
  if (inTri) return true;
  if (d < 5.6 || d > 11.6) return false;
  if (Math.hypot(cx - 19.8, cy - 4.2) < 4.3 || Math.hypot(cx - 15.6, cy - 1.4) < 2.8 || Math.hypot(cx - 22.8, cy - 8.3) < 2.6) return false;
  return true;
};
let rects = "";
for (let y = 0; y < 24; y++) for (let x = 0; x < 24; x++) if (on(x, y)) rects += `<rect x="${x + 4}" y="${y + 4}" width="1" height="1"/>`;
const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" shape-rendering="crispEdges"><rect width="32" height="32" fill="#000"/><g fill="#fff">${rects}</g></svg>`;
fs.writeFileSync(".icon/logo.svg", svg);
fs.writeFileSync(".icon/logo-1024.png", new Resvg(svg, { fitTo: { mode: "width", value: 1024 } }).render().asPng());
fs.writeFileSync("static/favicon.png", new Resvg(svg, { fitTo: { mode: "width", value: 64 } }).render().asPng());
