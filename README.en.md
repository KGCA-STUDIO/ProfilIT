# ProfileIT

*[한국어](README.md)*

A static-site generator for section-based online business cards. Edit one
config file and get a set of static files in `dist/` that you can drop
straight onto GitHub Pages or anywhere else.

Mix and stack education/career timelines, a bucket list, interest tags, link
cards, and contact info — visitors can save the contact info straight to
their address book as a `.vcf`. Cards can be built in multiple languages,
with a language switcher on the page.

## Using it

The desktop app is the easier way to go.

```bash
cargo run -p profileit-desktop
```

If you'd rather stay in the terminal, the CLI is right there too.

```bash
cargo run -- edit           # browser editor UI (local only)
cargo run -- build          # profile.toml → dist/
cargo run -- check          # validate only
cargo run -- init           # scaffold a new profile.toml
cargo run -- deploy         # build, then push to GitHub Pages
```

## Deploying to GitHub Pages

The editor's **Deploy to GitHub** button (or `cargo run -- deploy`) pushes
`dist/` to the `gh-pages` branch. It always rebuilds before pushing, so you
never deploy with unsaved changes missing.

### Connecting GitHub

The **Connect GitHub** button at the top walks you through the steps.

1. Opens the token-creation screen (the required `repo` scope is
   pre-selected)
2. Paste the token you created and it verifies your account
3. Pick a repo from the list or create a new one — the remote is set up
   automatically
4. Click **Deploy to GitHub** to push and **Pages gets turned on
   automatically too**

The token is stored in the **OS credential store** (Windows Credential
Manager, macOS Keychain, Linux Secret Service). It's never written to a
config file or app folder in plain text, so sharing the whole project
folder — or accidentally committing it — won't leak the token along with it.

Pushes pass the token through `GIT_ASKPASS` rather than the command line
(`https://token@...`), since that would expose it in the process list and in
git's own error output. If it ever ends up mixed into an error message
anyway, it's scrubbed before it reaches the screen.

Opening an external URL in the browser **never goes through a shell.** On
Windows, `cmd /C start` reinterprets the command line by its own rules, so
`&`, `|`, and `%` in a URL would act as command separators or environment
variable expansions — a single URL could end up launching a different
program.

The temporary askpass script is written inside the repository's `.git`
directory, not the shared system temp folder. Unix's `/tmp` is writable by
anyone, so a predictable filename there lets an attacker pre-place a symlink
to overwrite an unrelated file, or swap the contents out before git actually
runs it, running arbitrary code with the user's privileges.

You don't have to connect an account to use this. Without it, pushes use
whatever git credentials are already configured, and you create the repo and
turn on Pages yourself on github.com.

> **Real OAuth login** (an Authorize button) needs a registered OAuth App.
> Once you have a `client_id`, add the device-authorization flow to
> `src/github.rs`; everything else (listing/creating repos, enabling Pages)
> already works as-is.

The working tree is never touched. Instead of checking out a branch, it
builds `dist/` into a temporary index and forges a commit with `commit-tree`
before pushing — so files you're mid-edit on, or anything staged, are
unaffected by a deploy. If `site.base_url` doesn't match the real address,
you get a warning after deploying — this catches the kind of bug where the
page looks fine but the share card points somewhere wrong, before anyone
notices by sharing it.

The output is just `index.html`, `styles.css`, `card.js`, plus whatever
assets your config references. No server required. With multiple languages,
the default language sits at `/` and the rest live under subpaths like
`/en/`, with a single shared copy of the assets at the root.

## Desktop app

[`src-tauri/`](src-tauri) is the Tauri shell. It only handles the window and
menu — editing, validation, and rendering all come from the `profileit`
library. **The reason editing logic doesn't live in the app** is that it
would otherwise end up with validation rules in two places that quietly
drift apart.

Sharing a process between the app and the library buys two things.

The edit server runs on a single thread. There's no spawning a child
process, handing off a port, and waiting for it to come up — and when the
app quits, the server goes with it, so no orphaned processes are left
behind.

Switching card folders doesn't restart the server. `Editor::open` just
updates the server's state and reloads the window.

- **File → Open Card Folder** (Ctrl+O) — switches to a different folder. If
  it has no `profile.toml`, you're asked whether to create one and a starter
  file is written. Existing files are never overwritten
- **File → Open Generated Site Folder** — opens `dist/` in the file explorer
- The last folder you opened is remembered and reopened automatically next
  time

**WebView2 Runtime** is required on Windows. It's usually already installed
on Windows 10/11.

To package it as an executable, flip `bundle.active` to `true` in
`tauri.conf.json` and run `tauri build` with the
[Tauri CLI](https://v2.tauri.app/reference/cli/).

## Editor UI

`cargo run -- edit` starts a small server on `127.0.0.1` and opens a
browser. The left side is a form; the right side is an iframe showing
**the actual renderer output**, so the preview can never drift from what
gets built. Tabs at the top pick which language you're editing.

Profile photos and link thumbnails can be uploaded directly by picking a
file. They're saved into `assets/` and a relative path goes into the
config. Both the extension and the file contents are checked, so a file
that's just had its extension changed gets rejected (8MB max,
png/jpg/gif/webp/svg).

Saving **merges** into the file rather than overwriting it
(via `toml_edit`), so comments, field order, and arrays you've formatted
across multiple lines all stay intact. If there's a validation error,
nothing is saved and the reason is shown at the bottom of the screen.

Two things worth knowing.

Defaults you'd left out get written explicitly on the first save (like
`enabled = true`). That's because the editor sends the whole config — if
defaults were omitted instead, a deliberately-set value like `done = false`
could get silently erased too, so this tradeoff was chosen deliberately.
From the second save on, the file stops changing on its own.

Comments attach to position, not content. If you reorder sections in the
editor, a comment stays where it was and ends up describing the wrong
section.

You can build without the editor if you don't need it.

```bash
cargo build --no-default-features
```

## Customizing

Everything about the content and look lives in one file,
[`profile.toml`](profile.toml). See [`docs/schema.md`](docs/schema.md) for
the full field list.

```toml
[[sections]]
type = "timeline"
title = "Education"
icon = "🎓"

[[sections.items]]
period = "2014 – 2018"
title = "Example University, Computer Science"
subtitle = "B.S."
```

To hide a section temporarily, don't delete it — set `enabled = false`
instead. Most items support the same field.

Backgrounds can be a solid color, gradient, pattern, or image, and fonts are
picked by name from a preset of Korean-friendly web fonts — no need to go
hunting for CDN URLs.

```toml
[theme.background]
type = "pattern"
name = "dots"

[theme.font]
preset = "pretendard"
heading_preset = "jua"     # a different font for headings only
```

## Multiple languages

Only values that need translating get turned into a table. Everything else
stays a plain string and is shared across every language.

```toml
[site]
lang = "ko"                  # default language, served at the site root (/)
languages = ["ko", "en"]     # the rest live under /en/, etc.

[[sections]]
type = "timeline"
title = { ko = "학력", en = "Education" }
```

Translations live inline in the config rather than in a separate file
because sitting right next to the original text makes a missed one easy to
spot. A path-based approach (`sections[1].title`) silently breaks the moment
sections get reordered.

Screen text like "Save Contact" lives in `locales/<code>.json`. Korean,
English, and Japanese ship built in, and dropping in a file with the same
name in your project overrides those strings — you don't have to write out
the whole file just to change one line.

**The editor's own UI also switches between those same three languages.**
Pick one with `KO EN JA` in the top right, and this machine remembers the
choice. On first open it follows the browser's language. This is
**independent** of the card's content language — writing an English card
from a Korean-language editor UI is a common case. Editor strings live in
the same locale files, under keys prefixed with `editor.`.

**Text the server sends — validation errors, deploy errors — switches
too.** The server sends a key and arguments (prefixed with `msg.`) rather
than a finished sentence, and the screen builds the sentence in its own
language. The terminal (`profileit build`, etc.) and the desktop app's menu
move independently of the screen: they use `PROFILEIT_LANG` if it's set, and
otherwise fall back to the card's default language — the menu is drawn once
when the window is created, so it only updates on the next app launch.

Missing translations fall back to the default language, and validation
rolls them up into **one line per language**.

```
[warning] site.languages (en): 6 items are missing an en translation and
          fall back to the default language (ko): site.title,
          site.description, profile.name, ... and 2 more
```

## Validation

`build` won't run unless validation passes — it's better for the build to
stop than for a broken result to get deployed.

**Errors** block the build — a missing file, a dangerous URL like
`javascript:`, an invalid color value, a malformed email or phone number, a
CSS length with no unit.

**Warnings** let the build through but flag things that probably weren't
intended — a typo'd section key, a font weight the font doesn't have, a
pattern color identical to the background, an item marked `done = false`
that still has a completion date.

Locations are shown as a TOML path, like `sections[2].items[0].url`.

## Structure

| File | Role |
|---|---|
| `src/config.rs` | canonical schema definitions |
| `src/validate.rs` | validation rules |
| `src/theme.rs` | `[theme]` → CSS custom properties |
| `src/render.rs` | maud templates |
| `src/icons.rs` · `src/decor.rs` | built-in SVGs |
| `src/i18n.rs` | translatable values (`Text`) and UI strings |
| `src/message.rs` | key+args for user-facing text (`Message`) |
| `src/build.rs` | generates `dist/` |
| `src/save.rs` | comment-preserving save (editor only) |
| `src/serve.rs` | local edit server (editor only) |
| `src/lib.rs` | library entry point — shared by the CLI and the app |
| `src/init.rs` | starter file for a new `profile.toml` |
| `src/deploy.rs` | GitHub Pages deployment |
| `src/github.rs` | GitHub integration (token storage, repos, Pages) |
| `static/` | stylesheets/scripts/editor embedded in the binary |
| `locales/` | UI strings |
| `src-tauri/` | desktop app (Tauri) |

The schema **doesn't depend on how it's edited.** The static renderer and
the local editor UI share the same types and the same validation rules —
and that'll still hold if a browser-based admin mode gets added later.

`[theme]` fields map 1:1 to CSS custom properties. `styles.css` only ever
*consumes* `--pf-*` tokens, so changing the theme leaves the stylesheet
untouched in cache while the editor UI just swaps variables to produce a
live preview.

## Contributing

Icons and font presets are intentionally easy to extend.

- **Platform icons** — add a variant to `Platform` and a matching arm in
  `icons::platform`
- **Font presets** — add a variant to `FontPreset` and fill in `family`,
  `stylesheet`, and `available_weights`
- **Decoration presets** — add a variant to `DecorationPreset` and extend
  `decor::top`/`decor::bottom`
- **Languages** — create `locales/<code>.json` and add one line to
  `i18n::BUILTIN`. Tests catch any missing keys

```bash
cargo test
```

## License

GPL-3.0-or-later. Full text in [`LICENSE`](LICENSE).
