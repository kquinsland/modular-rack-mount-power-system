{{- $title := or .Title .LinkTitle .Site.Title (path.Base (strings.TrimSuffix "/" .RelPermalink) | humanize | title) -}}

# {{ $title }}

{{ with .Description }}{{ . }}

{{ end -}}
Canonical: {{ .Permalink }}
{{ if not .Date.IsZero }}Published: {{ .Date.Format "2006-01-02" }}
{{ end -}}
{{ if not .Lastmod.IsZero }}Updated: {{ .Lastmod.Format "2006-01-02" }}
{{ end -}}
{{ with .Params.tags }}Tags: {{ delimit . ", " }}
{{ end -}}
{{ with .File }}
{{- $ref := "main" -}}
{{- with $.GitInfo }}{{ $ref = .Hash }}{{ end -}}
{{- $sourcePath := printf "site/content/%s" (.Path | replaceRE `\\` "/") -}}
{{- $sourcePath = replace $sourcePath " " "%20" }}
Source: {{ site.Params.github_source_base }}/{{ $ref }}/{{ $sourcePath }}
{{ end }}

---

{{ .RenderShortcodes }}
