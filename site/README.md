# Documentation site

This directory contains the Hugo source for <https://mrp.karlquinsland.com/>.
The site uses the pinned HuDocs submodule in `themes/hudocs`.

## Local development

From the repository root:

```sh
mise run site:init
mise run site:serve
```

The build and development-server tasks also run `site:init` automatically, so the pinned theme is available before Hugo starts.

Run `mise run site:build` to validate the source and create a production build in `site/public/`.

Create a worklog with:

```sh
mise run worklog:new -- "Concise Summary"
```

The helper creates `site/content/worklogs/wl.YYYY-MM-DD - Concise Summary.md`.
The filename date and front-matter date must agree.

## Content organization

The permanent top-level sections are `system/`, `guides/`, and `worklogs/`.
Git history is the archive for older versions of the documentation.

Use Hugo page bundles for nested documentation: branch bundles for sections and
leaf bundles for standalone pages. Keep images, generated metadata, and other
supporting files in an owning page's `_files/` directory and reference them
with relative paths. Reserve `static/` for shared assets and files Hugo would
otherwise interpret as content.

PCB images are generated with `mise run docs:pcb-renders` and stored as WebP.
Carrier and backplane `_files/` directories contain top and bottom views plus
`renders.json`; `system/hardware/_files/` owns `panel.webp`.

The `mise run docs:pcb-iboms` task publishes interactive assembly HTML under
`static/guides/assembly/_files/`, which serves it at
`/guides/assembly/_files/`. HTML is kept under `static/` because Hugo otherwise
interprets it as content.

Preview images do not imply that the PCB passed release checks.
See the [release tooling guide](../.kibot/release/README.md) for the validated build.

## Deployment

The [Pages workflow](../.github/workflows/pages.yml) is intentionally manual, initially.
Inspect repository state with the authenticated GitHub CLI before changing Pages configuration:

```sh
export GH_REPO="kquinsland/modular-rack-power"
gh repo view
gh api "repos/${GH_REPO}/pages"
```

Once Pages uses GitHub Actions, dispatch the workflow with the Git ref to
publish.
DNS for `mrp.karlquinsland.com` is managed outside this repository.
