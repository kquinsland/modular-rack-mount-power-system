{{- $figure := partial "figure-data.html" . -}}
{{- $alt := $figure.alt | plainify | replaceRE `([\\\[\]])` `\$1` -}}
{{- $image := printf "![%s](<%s>)" $alt (absURL $figure.src) -}}
{{- with $figure.link -}}
  {{- $link := . -}}
  {{- if and (not (urls.Parse $link).IsAbs) (not (hasPrefix $link "/")) -}}
    {{- $link = printf "%s%s" $.Page.Permalink $link -}}
  {{- end -}}
  {{- $image = printf "[%s](<%s>)" $image (absURL $link) -}}
{{- end -}}
{{ $image }}
{{ if and $figure.showTitle $figure.title }}
**{{ $figure.title | plainify }}**
{{ end -}}
{{ with $figure.caption }}
{{ . }}
{{ end -}}
{{ with $figure.attr }}
{{ if $figure.attrlink }}[{{ . | plainify }}](<{{ $figure.attrlink | absURL }}>){{ else }}{{ . | plainify }}{{ end }}
{{ end -}}
