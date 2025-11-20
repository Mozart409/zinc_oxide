import { Hono } from 'hono'
import { trimTrailingSlash } from 'hono/trailing-slash'

const app = new Hono({strict: true})

app.use(trimTrailingSlash())

app.get('/api/v1', (c) => {
  return c.text('Hello Hono!')
})

export default app
