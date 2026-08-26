# Maju Entity Links

Maju uses canonical `maju://` deep links for repositories, projects, pull
requests, and issues. They are shareable inside a community and open the
matching Projects view in Maju Desktop.

## Link format

```text
maju://repo?owner=<pubkey-hex>&d=<repo-dtag>[&tab=<tab>][&commit=<git-hash>]
maju://project?owner=<pubkey-hex>&d=<project-dtag>[&tab=<tab>]
maju://pr?id=<event-id-hex>&owner=<pubkey-hex>&d=<repo-dtag>
maju://issue?id=<event-id-hex>&owner=<pubkey-hex>&d=<repo-dtag>
```

- `owner` is the lowercase 64-character public key of the repository or
  project announcement author.
- `d` is the addressable event's `d` tag.
- `id` is the lowercase 64-character pull request or issue event id.
- `tab` may select `files`, `commits`, `issues`, `prs`, `contributors`, or
  `channels` for repository and project links.
- `commit` is accepted only for repository links on the `commits` tab and is a
  40- or 64-character hexadecimal Git object id.

The link is community-relative. The receiving client resolves it against the
community where the message appears. Cross-community `relay=` parameters are
not emitted or accepted.

## Chat presentation

Bare entity links render as compact inline chips. Explicitly labelled Markdown
links keep the author's label as an inline link. Hovering either presentation
shows relay-backed context such as the entity title and containing project.

Maju does not add a second attachment card for these links. This keeps the
entity visible at the exact point where the author placed it and avoids showing
the same destination twice.

Clicking the chip or link navigates in-app. Right-clicking offers Open link and
Copy link. An HTTPS clone URL for the active relay with the shape
`/git/<owner-pubkey>/<repo>` is normalized to the corresponding repository deep
link and uses the same in-app behavior.

## Navigation

Repository-scoped links resolve through the canonical
`30617:<owner>:<d>` coordinate. Project links use
`30621:<owner>:<d>`.

- A repository opens its project workspace and optional selected tab or
  commit.
- A pull request or issue opens the containing repository workspace with that
  event selected.
- A project opens the project workspace and optional selected tab.

The OS deep-link handler validates the same canonical format before focusing
the app and routes accepted links through the same navigation path as links
clicked in a message.

## CLI and agent guidance

The create commands below include a `link` field in their JSON response:

- `maju pr open`
- `maju issues create`
- `maju repos create`
- `maju projects create`

Agents should include that value verbatim when announcing the created item.
Maju-hosted pull requests and issues do not have an invented public HTTPS page;
the `maju://` link and a repository's clone URL are the shareable references.

Rust builders live in `crates/maju-cli/src/links.rs`. Their TypeScript mirror
lives in `desktop/src/shared/lib/entityLink.ts`. Golden-format tests on both
sides must stay compatible.

## Security

- Builders and parsers validate all identifiers before rendering a clickable
  target.
- Invalid links remain plain text and never navigate.
- Hover metadata is loaded through the authenticated relay client with
  explicit event-kind filters.
- Relay-provided titles and descriptions are rendered as text, never as raw
  HTML.
- OS deep links are untrusted input and pass through the same parser as chat
  links.
