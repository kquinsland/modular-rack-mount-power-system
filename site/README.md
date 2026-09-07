# Documentation site

This directory contains the Hugo source for <https://mrp.karlquinsland.com/>.
The site uses the pinned HuDocs submodule in `themes/hudocs`.

## Local development

From the repository root:

```sh
git submodule update --init --recursive
mise run site:serve
```

Run `mise run site:build` to validate the source and create a production build
in `site/public/`.

Create a worklog with:

```sh
mise run worklog:new -- "Concise Summary"
```

The helper creates `site/content/worklog/wl.YYYY-MM-DD - Concise Summary.md`.
The filename date and front-matter date must agree.

## Content organization

Use Hugo page bundles for documentation with page-specific assets. Leaf pages
use `page-name/index.md`; section pages remain branch bundles with `_index.md`.
Store renders and other page-specific files beside the owning Markdown file and
reference them with relative paths. Reserve `static/` for assets shared across
multiple pages.

PCB images are generated with `mise run docs:pcb-renders` and stored as WebP
beside each board's Markdown. Carrier and backplane bundles contain top and
bottom views plus `renders.json`; the hardware section owns `panel.webp`.
`mise run docs:pcb-iboms` also publishes interactive assembly HTML under
`static/assembly/` (Hugo otherwise interprets HTML as content). Preview
images do not imply that the PCB passed release checks. See the
[release tooling guide](../.kibot/release/README.md) for the validated build.

## Documentation versions

`content/latest/` is the moving technical reference. Worklogs live separately
and are never versioned. A future release process may copy `latest` to an
immutable version directory and add that version to `params.versions` in
`hugo.toml`.

## Deployment

The Pages workflow is intentionally manual. Inspect repository state with the
authenticated GitHub CLI before changing Pages configuration:

```sh
export GH_REPO="kquinsland/modular-rack-power"
gh repo view
gh api "repos/${GH_REPO}/pages"
```

Once Pages uses GitHub Actions, dispatch the workflow with the Git ref to
publish. DNS for `mrp.karlquinsland.com` is managed outside this repository.
