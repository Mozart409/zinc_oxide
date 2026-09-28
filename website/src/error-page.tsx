type ErrorPageProps = {
  status: number;
  title: string;
  message: string;
};

/** Generic error page, styled and themed like the docs page. */
export const ErrorPage = ({ status, title, message }: ErrorPageProps) => (
  <html lang="en">
    <head>
      <title>{`${status} ${title} · Zinc Oxide`}</title>
      <meta charset="UTF-8" />
      <meta name="viewport" content="width=device-width, initial-scale=1.0" />
      <meta name="robots" content="noindex" />
      <script src="/theme.js"></script>
      <link rel="stylesheet" href="/reset.css" />
      <link rel="stylesheet" href="/style.css" />
      <link
        rel="preconnect"
        href="https://api.fonts.coollabs.io"
        crossorigin=""
      />
      <link
        href="https://api.fonts.coollabs.io/css2?family=IBM+Plex+Mono:wght@300;400&display=swap"
        rel="stylesheet"
      />
    </head>
    <body>
      <main class="container error-page">
        <p class="error-status ibm-plex-mono-light">{status}</p>
        <h1 class="ibm-plex-mono-regular">{title}</h1>
        <p class="ibm-plex-mono-regular">{message}</p>
        <p class="ibm-plex-mono-regular">
          <a href="/">Back to the documentation</a>
        </p>
      </main>
    </body>
  </html>
);

export const NotFoundPage = () => (
  <ErrorPage
    status={404}
    title="Page not found"
    message="There is nothing at this address. It may have moved, or the link may be wrong."
  />
);

export const ServerErrorPage = () => (
  <ErrorPage
    status={500}
    title="Something went wrong"
    message="The server hit an unexpected error. Please try again in a moment."
  />
);
