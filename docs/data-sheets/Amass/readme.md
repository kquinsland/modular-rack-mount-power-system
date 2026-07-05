# Amass XT30 naming scheme

The good news is that their model numbers encode all the useful information.
The bad news is that they do so cryptically.

Reading through a few data-sheets I think I figured out how

```
- XT30PW(2+2)-M.G.B.pdf
- XT30U(2+2)-F.G.B.pdf
```

| Code  | Meaning                                                     | Confidence |
| ----- | ----------------------------------------------------------- | ---------- |
| XT30  | 30A connector family                                        | 100%       |
| (2+2) | 2 power + 2 signal                                          | 100%       |
| U     | SMT board connector                                         | 99%        |
| PW    | Panel-mount connector                                       | ~90%       |
| PB    | Cable plug / PCB-mounted mating half (depending on variant) | ~80%       |
| M     | Male                                                        | 100%       |
| F     | Female                                                      | 100%       |
| G     | Gold plating                                                | 100%       |
| B     | Black housing                                               | 100%       |
