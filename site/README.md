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

The helper creates `site/content/worklogs/YYYY/MM/DD - Concise Summary/index.md`
using today's date, plus an adjacent `_files/` directory for images and other
page resources. For example:

```text
site/content/worklogs/2026/09/10 - First Physical Prototypes/
├── index.md
└── _files/
    ├── early_prototype_01.webp
    └── early_prototype_02.webp
```

The bundle's year/month/day and front-matter date must agree. New entries start
as drafts with an empty `resources` list; add image metadata there and insert
the `figure` shortcodes described below. The generated slug keeps the day and
summary in the URL, e.g. `/worklogs/2026/09/10-first-physical-prototypes/`.

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

## Callouts and figures

Use GitHub-style callouts in Markdown:

```markdown
> [!WARNING]
> Everything about this project is still under active development.
```

`NOTE`, `TIP`, `IMPORTANT`, `WARNING`, and `CAUTION` have distinct colors and
icons in both light and dark mode. Callout bodies support normal Markdown,
including lists and multiple paragraphs. Ordinary blockquotes keep their usual
appearance. The implementation uses Hugo's
[blockquote render hook](https://gohugo.io/render-hooks/blockquotes/).

The `figure` shortcode accepts the same named page resources as the blog.
Define image metadata in the owning page's front matter:

```yaml
resources:
  - src: _files/carrier.webp
    name: carrier-top
    title: Carrier, top side
    params:
      alt: Carrier PCB viewed from above
      caption: "Generated preview; see [render provenance](_files/renders.json)."
      attr: Karl Quinsland
      attr_link: https://karlquinsland.com/
```

Then insert the image:

```go-html-template
{{< figure name="carrier-top" >}}
{{< figure name="carrier-top" show_title="true" link="_files/carrier.webp" >}}
```

Titles are hidden by default for named resources, matching the blog. Captions
support Markdown. Optional shortcode arguments override resource metadata:
`title`, `alt`, `caption`, `attr`, and `attr_link` (or `attrlink`). An explicit
`alt=""` marks a decorative image. `class`, `width`, `height`, and `loading`
are also available; images are responsive and lazy-loaded by default.
Missing named resources fail the build with the shortcode's source location.

For a direct image path or URL, use `src` instead of `name`:

```go-html-template
{{< figure src="_files/carrier.webp" alt="Carrier PCB" title="Carrier" caption="Top view" >}}
```

Both HTML and the site's alternate Markdown output include the image and its
caption, visible title, attribution, and optional link.

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
