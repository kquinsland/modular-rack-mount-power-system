{{- $bundle := path.Base (path.Dir .File.Path) -}}
{{- $date := .File.Path | replaceRE `^worklogs/([0-9]{4})/([0-9]{2})/([0-9]{2}) - .*/index\.md$` `${1}-${2}-${3}` -}}
---
title: '{{ $bundle | replaceRE `^[0-9]{2} - ` "" | replaceRE `'` `''` }}'
date: {{ $date }}
description: ""
tags: []
slug: '{{ $bundle | replaceRE ` - ` "-" | urlize }}'
draft: true
resources: []
---

Write the concise project update here.
