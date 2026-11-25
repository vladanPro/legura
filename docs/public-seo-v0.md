# Public SEO V0

Published detail routes under `/posts/` and `/pages/` render metadata in the
server-generated HTML head. JavaScript is not required.

- Title: content title and site title, separated by a vertical bar.
- Description: the published plain-text body, with normalized whitespace and
  a 160-character cap. Rich posts use their stored plain-text projection.
- Canonical URL: the configured public **Site Address**, including any base
  path, followed by the published route. If the address is blank or invalid,
  no canonical tag is emitted. Request hosts and query parameters are not used.

Drafts, missing content and admin pages do not receive the public metadata.
No new database columns, editor fields, sitemap, or social preview tags are
introduced. The compiled `-Posts` smoke checks the emitted HTML and boundaries
using the published Axonyx Runtime 0.8.7.
