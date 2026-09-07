---
title: '{{ .Name | replaceRE `^wl\.[0-9]{4}-[0-9]{2}-[0-9]{2} - ` "" }}'
date: {{ .Date }}
description: ""
tags: []
slug: '{{ .Name | replaceRE `^wl\.` "" | replaceRE ` - ` "-" | urlize }}'
draft: true
---

Write the concise project update here.
