import { Hono } from "hono";
import { html } from "hono/html";
import { trimTrailingSlash } from "hono/trailing-slash";
import { NotFoundPage, ServerErrorPage } from "./error-page";

const app = new Hono({ strict: true });

app.use(trimTrailingSlash());

app.get("/api/v1", (c) => {
  return c.text("Hello Hono!");
});

// Static assets are served before the worker runs, so anything that reaches
// here without a route is a missing page.
app.notFound((c) => c.html(html`<!DOCTYPE html>${<NotFoundPage />}`, 404));

app.onError((err, c) => {
  console.error(err);
  return c.html(html`<!DOCTYPE html>${<ServerErrorPage />}`, 500);
});

export default app;
