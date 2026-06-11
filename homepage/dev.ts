// Dev server: builds, serves on :4321, and rebuilds + live-reloads on change.
//   bun run dev

import { watch } from "node:fs";

const PORT = 4321;
const DIST = "dist";

async function build(): Promise<number> {
  const t = performance.now();
  const proc = Bun.spawn(["bun", "run", "build.ts"], { stdout: "inherit", stderr: "inherit" });
  await proc.exited;
  return Math.round(performance.now() - t);
}

const clients = new Set<ReadableStreamDefaultController<Uint8Array>>();
const ping = new TextEncoder().encode("data: reload\n\n");
const reloadAll = () => {
  for (const c of clients) {
    try {
      c.enqueue(ping);
    } catch {
      /* client gone */
    }
  }
};

console.log(`first build in ${await build()}ms`);

Bun.serve({
  port: PORT,
  async fetch(req) {
    const path = new URL(req.url).pathname;

    // Live-reload channel.
    if (path === "/__livereload") {
      let ctl: ReadableStreamDefaultController<Uint8Array> | undefined;
      const body = new ReadableStream<Uint8Array>({
        start(c) {
          ctl = c;
          clients.add(c);
        },
        cancel() {
          if (ctl) clients.delete(ctl);
        },
      });
      return new Response(body, {
        headers: { "content-type": "text/event-stream", "cache-control": "no-cache" },
      });
    }

    // Inject the reload client into the page (dev only — the built file stays clean).
    if (path === "/" || path === "/index.html") {
      const html = (await Bun.file(`${DIST}/index.html`).text()).replace(
        "</body>",
        `<script>new EventSource("/__livereload").onmessage=function(){location.reload()}</script></body>`,
      );
      return new Response(html, { headers: { "content-type": "text/html; charset=utf-8" } });
    }

    const file = Bun.file(DIST + path);
    return (await file.exists()) ? new Response(file) : new Response("Not found", { status: 404 });
  },
});
console.log(`dev -> http://localhost:${PORT}`);

let pending: ReturnType<typeof setTimeout> | undefined;
const schedule = (file: string) => {
  clearTimeout(pending);
  pending = setTimeout(async () => {
    console.log(`rebuild (${file}) in ${await build()}ms`);
    reloadAll();
  }, 80);
};
for (const target of ["public", "build.ts", "content.ts"]) {
  watch(target, { recursive: true }, (_event, file) => file && schedule(String(file)));
}
