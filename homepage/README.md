# Polaris homepage

The marketing/landing page for Polaris, deployed to **Cloudflare Pages**. Built
with **[Bun](https://bun.sh)**.

## How it works

- **Build-time rendering, ~no JS** — `build.ts` renders a fully static
  `dist/index.html` with **both English and Chinese** content baked in. There is
  no framework and **no JavaScript bundle** — the page is styled in the **Fluent
  design language** with plain CSS, and the only script is ~0.5 KB inline for the
  language switch.
- **Language: auto + manual** — an inline script picks the language before first
  paint (a saved preference, else the browser UI language → Chinese gets `zh`,
  otherwise English); CSS shows only the active language. The header button
  toggles and remembers the choice.
- **Theme** — light/dark follows the OS via a pure-CSS `prefers-color-scheme`
  media query (no JS), so cards and buttons are correct in dark mode.

Content lives in [`content.ts`](content.ts) — edit it there; both languages are
required for each string. Styling is in [`public/styles.css`](public/styles.css).

## Develop

```sh
bun install        # dev tooling only (@types/bun); the site has no runtime deps
bun run dev        # build + serve on :4321 with live reload
```

Other scripts:

```sh
bun run build      # static site -> ./dist
bun run preview    # build, then serve ./dist on :4321 (no reload)
```

## Deploy (Cloudflare Pages)

Point Cloudflare Pages at this repository and set:

| Setting | Value |
| --- | --- |
| **Root directory** | `homepage` |
| **Build command** | `bun run build` |
| **Build output directory** | `dist` |
| **Framework preset** | None |

Bun is auto-detected from the committed `bun.lock`. The output in `dist/` is
plain static files (`index.html`, `styles.css`, `data-line.svg`), so it works on
any static host.
