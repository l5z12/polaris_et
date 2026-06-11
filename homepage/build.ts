// Build-time render: emits a fully static dist/index.html with BOTH languages
// baked in and styled in the Fluent design language (CSS only — no component
// runtime). The only JavaScript is a tiny inline language switch; the theme is
// pure CSS. Output target: Cloudflare Pages.
//
//   bun run build   ->   ./dist

import { copyFile, mkdir, readdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { REPO, RELEASES, type Bi, site } from "./content.ts";

const OUT = "dist";

const esc = (s: string) =>
  s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");

// Render both languages; CSS hides whichever isn't active.
const bi = (b: Bi) =>
  `<span lang="en">${esc(b.en)}</span><span lang="zh">${esc(b.zh)}</span>`;

const featureCard = (f: { title: Bi; body: Bi }) =>
  `<article class="card"><h3>${bi(f.title)}</h3><p>${bi(f.body)}</p></article>`;

// Auto-pick the language before first paint (saved pref, else browser UI
// language → Chinese gets zh), then the toggle just flips + remembers it.
const LANG_SCRIPT = `(function(){try{var s=localStorage.getItem("lang");var l=s||((((navigator.language||"")+"").toLowerCase().indexOf("zh")===0)?"zh":"en");var h=document.documentElement;h.setAttribute("data-lang",l);h.lang=l;}catch(e){}})();`;
const TOGGLE_SCRIPT = `document.getElementById("lang-toggle").addEventListener("click",function(){var h=document.documentElement,n=h.getAttribute("data-lang")==="zh"?"en":"zh";h.setAttribute("data-lang",n);h.lang=n;try{localStorage.setItem("lang",n)}catch(e){}});`;

function page(): string {
  const features = site.features.map(featureCard).join("\n        ");
  return `<!doctype html>
<html lang="en" data-lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>${esc(site.name)}</title>
<meta name="description" content="${esc(site.description)}" />
<meta name="color-scheme" content="light dark" />
<link rel="icon" type="image/svg+xml" href="/data-line.svg" />
<link rel="stylesheet" href="/styles.css" />
<script>${LANG_SCRIPT}</script>
</head>
<body>
  <header class="topbar">
    <a class="brand" href="${REPO}"><img src="/data-line.svg" width="24" height="24" alt="" /><span>${esc(site.name)}</span></a>
    <button id="lang-toggle" class="btn subtle" type="button" aria-label="Switch language">${bi(site.switchTo)}</button>
  </header>

  <main>
    <section class="hero">
      <img class="hero-logo" src="/data-line.svg" width="104" height="104" alt="" />
      <h1>${esc(site.name)}</h1>
      <p class="tagline">${bi(site.tagline)}</p>
      <div class="cta">
        <a class="btn accent" href="${RELEASES}">${bi(site.download)}</a>
        <a class="btn neutral" href="${REPO}">${bi(site.viewSource)}</a>
      </div>
    </section>

    <section class="features">
      <h2>${bi(site.featuresHeading)}</h2>
      <div class="grid">
        ${features}
      </div>
    </section>

    <footer class="foot">
      <p>${bi(site.requirements)}</p>
      <p>${bi(site.license)} · <a class="link" href="${REPO}/blob/main/LICENSE">GNU GPL v3.0</a></p>
    </footer>
  </main>

  <script>${TOGGLE_SCRIPT}</script>
</body>
</html>
`;
}

await rm(OUT, { recursive: true, force: true });
await mkdir(OUT, { recursive: true });

for (const name of await readdir("public")) {
  await copyFile(join("public", name), join(OUT, name));
}

await writeFile(join(OUT, "index.html"), page());
console.log(`built -> ${OUT}/  (no JS bundle)`);
