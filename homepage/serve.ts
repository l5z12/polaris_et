// Local preview of the built site: `bun run preview`.
const DIST = "dist";
const PORT = 4321;

Bun.serve({
  port: PORT,
  async fetch(req) {
    const path = new URL(req.url).pathname;
    const file = Bun.file(DIST + (path === "/" ? "/index.html" : path));
    return (await file.exists()) ? new Response(file) : new Response("Not found", { status: 404 });
  },
});

console.log(`preview -> http://localhost:${PORT}`);
