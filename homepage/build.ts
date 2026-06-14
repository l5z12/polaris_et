// Build-time render: emits a fully static dist/index.html with BOTH languages
// baked in and styled in the Fluent design language (CSS only — no component
// runtime). The only JavaScript is a tiny inline language switch; the theme is
// pure CSS. Output target: Cloudflare Pages.
//
//   bun run build   ->   ./dist

import { execSync } from "node:child_process";
import { copyFile, mkdir, readdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { REPO, RELEASES, type Bi, site } from "./content.ts";
import { EASYTIER_PRIVACY, privacy } from "./privacy.ts";

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

// Privacy "last updated" derived from git: the last commit that touched the
// policy source (privacy.ts). Falls back to the latest commit overall — which
// covers shallow CI clones (e.g. Cloudflare Pages) where HEAD didn't change the
// file — and finally today. %cs is the committer date as YYYY-MM-DD, already
// timezone-resolved, so there's no Date/tz drift to reason about.
function gitDate(file: string): string {
  const run = (args: string) => {
    try {
      return execSync(`git log -1 --format=%cs ${args}`, {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
      }).trim();
    } catch {
      return "";
    }
  };
  return run(`-- "${file}"`) || run("") || new Date().toISOString().slice(0, 10);
}

const PRIVACY_UPDATED: Bi = (() => {
  const [y, m, d] = gitDate("privacy.ts").split("-").map(Number);
  const en = new Intl.DateTimeFormat("en-GB", {
    day: "numeric",
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  }).format(new Date(Date.UTC(y, m - 1, d)));
  return { en: `Last updated: ${en}`, zh: `最后更新：${y} 年 ${m} 月 ${d} 日` };
})();

const featureCard = (f: { title: Bi; body: Bi }) =>
  `<article class="card"><h3>${bi(f.title)}</h3><p>${bi(f.body)}</p></article>`;

// Pick the language before first paint: a saved pref wins; otherwise fall back to
// `force` (language-locked pages like /zh/) or, when not forced, the browser UI
// language (Chinese → zh). The toggle then just flips + remembers it.
const langScript = (force?: "en" | "zh") => {
  const fallback = force
    ? `"${force}"`
    : `((((navigator.language||"")+"").toLowerCase().indexOf("zh")===0)?"zh":"en")`;
  return `(function(){try{var s=localStorage.getItem("lang");var l=s||${fallback};var h=document.documentElement;h.setAttribute("data-lang",l);h.lang=l;}catch(e){}})();`;
};
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
<script>${langScript()}</script>
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
      <p><a class="link" href="/privacy.html">${bi(site.privacyLink)}</a></p>
    </footer>
  </main>

  <script>${TOGGLE_SCRIPT}</script>
</body>
</html>
`;
}

// `force` locks the default language (for /en/ and /zh/ variants); undefined
// keeps the auto-detecting canonical page at /privacy.html. Asset/nav links are
// root-absolute, so the page renders identically from a subdirectory.
function privacyPage(force?: "en" | "zh"): string {
  const lang = force ?? "en";
  const paras = privacy.body.map((p) => `<p>${bi(p)}</p>`).join("\n      ");
  // EasyTier's policy lives at a different URL per language, so render one anchor
  // per language (CSS hides the inactive one) with {link} spliced into the copy.
  const easytier = (["en", "zh"] as const)
    .map((l) => {
      const [before, after] = privacy.easytier[l].split("{link}");
      const a = `<a class="link" href="${esc(EASYTIER_PRIVACY[l])}">${esc(privacy.easytierLink[l])}</a>`;
      return `<span lang="${l}">${esc(before ?? "")}${a}${esc(after ?? "")}</span>`;
    })
    .join("");
  return `<!doctype html>
<html lang="${lang}" data-lang="${lang}">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>${esc(privacy.title[lang])} · ${esc(site.name)}</title>
<meta name="description" content="${esc(privacy.metaDescription[lang])}" />
<meta name="color-scheme" content="light dark" />
<link rel="icon" type="image/svg+xml" href="/data-line.svg" />
<link rel="stylesheet" href="/styles.css" />
<script>${langScript(force)}</script>
</head>
<body>
  <header class="topbar">
    <a class="brand" href="/"><img src="/data-line.svg" width="24" height="24" alt="" /><span>${esc(site.name)}</span></a>
    <button id="lang-toggle" class="btn subtle" type="button" aria-label="Switch language">${bi(site.switchTo)}</button>
  </header>

  <main>
    <article class="prose">
      <h1>${bi(privacy.title)}</h1>
      <p class="muted">${bi(PRIVACY_UPDATED)}</p>
      ${paras}
      <p>${easytier}</p>
      <p>${bi(privacy.contact)}</p>
      <p><a class="link" href="/">${bi(privacy.back)}</a></p>
    </article>
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
// Privacy: an auto-detecting canonical page, plus language-locked variants for
// per-market Store listings (e.g. the zh-CN listing points at /zh/privacy.html).
await writeFile(join(OUT, "privacy.html"), privacyPage());
for (const lang of ["en", "zh"] as const) {
  await mkdir(join(OUT, lang), { recursive: true });
  await writeFile(join(OUT, lang, "privacy.html"), privacyPage(lang));
}
console.log(`built -> ${OUT}/  (index + privacy + {en,zh}/privacy, no JS bundle)`);
