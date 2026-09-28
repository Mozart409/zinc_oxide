import { afterAll, beforeAll, describe, expect, test, vi } from "vitest";
import { unstable_dev, type Unstable_DevWorker } from "wrangler";
import app from "../src/index";

// Runs the real worker with its static assets, like `wrangler dev`.
let worker: Unstable_DevWorker;

beforeAll(async () => {
  worker = await unstable_dev("src/index.tsx", {
    config: "wrangler.jsonc",
    experimental: { disableExperimentalWarning: true },
    logLevel: "none",
  });
}, 60_000);

afterAll(async () => {
  await worker?.stop();
});

describe("docs page", () => {
  test("serves the docs at /", async () => {
    const res = await worker.fetch("/");
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toContain("text/html");
    const body = await res.text();
    expect(body).toContain("<title>Zinc Oxide</title>");
    expect(body).toContain("nix profile install github:Mozart409/zinc_oxide");
  });

  test("offers three dark and two light themes", async () => {
    const body = await (await worker.fetch("/")).text();
    const group = (label: string) =>
      [...(body.match(new RegExp(`<optgroup label="${label}">([\\s\\S]*?)</optgroup>`))?.[1] ?? "")
        .matchAll(/value="(\w+)"/g)].map((m) => m[1]);
    expect(group("Dark")).toEqual(["zinc", "nord", "gruvbox"]);
    expect(group("Light")).toEqual(["paper", "solarized"]);
  });

  test("serves every local asset the page references", async () => {
    const body = await (await worker.fetch("/")).text();
    const assets = [...body.matchAll(/(?:href|src)="(\/[^"]+)"/g)].map((m) => m[1]);
    expect(assets).toEqual(expect.arrayContaining(["/theme.js", "/reset.css", "/style.css"]));
    for (const asset of assets) {
      const res = await worker.fetch(asset);
      expect(res.status, asset).toBe(200);
    }
  });

  test("defines every selectable theme in the stylesheet", async () => {
    const css = await (await worker.fetch("/style.css")).text();
    for (const theme of ["zinc", "nord", "gruvbox", "paper", "solarized"]) {
      expect(css).toContain(`[data-theme="${theme}"]`);
    }
  });
});

describe("worker routes", () => {
  test("answers the API", async () => {
    const res = await worker.fetch("/api/v1");
    expect(res.status).toBe(200);
    expect(await res.text()).toBe("Hello Hono!");
  });

  test("renders the 404 page for unknown paths", async () => {
    for (const path of ["/nope", "/docs/deeply/missing"]) {
      const res = await worker.fetch(path);
      expect(res.status, path).toBe(404);
      const body = await res.text();
      expect(body.startsWith("<!DOCTYPE html>"), path).toBe(true);
      expect(body).toContain("404 Page not found");
      expect(body).toContain("href=\"/style.css\"");
    }
  });

  test("renders the 500 page without leaking the error", async () => {
    app.get("/__test/throw", () => {
      throw new Error("secret failure detail");
    });
    // The handler logs the error server-side; capture it instead of printing it.
    const logged = vi.spyOn(console, "error").mockImplementation(() => {});
    const res = await app.request("/__test/throw");
    const errors = logged.mock.calls.map(([err]) => err);
    logged.mockRestore();
    expect(res.status).toBe(500);
    const body = await res.text();
    expect(body.startsWith("<!DOCTYPE html>")).toBe(true);
    expect(body).toContain("Something went wrong");
    expect(body).not.toContain("secret failure detail");
    expect(errors).toEqual([
      expect.objectContaining({ message: "secret failure detail" }),
    ]);
  });
});
