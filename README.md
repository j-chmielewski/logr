# logr

TUI log viewer that highlights multiple regex patterns in streaming logs and lets you manage filters at runtime.

[![asciinema demo](https://asciinema.org/a/AdNygDJVw4N6TsMU.svg)](https://asciinema.org/a/AdNygDJVw4N6TsMU)

## Features

- Highlight multiple regex patterns with distinct colors
- Toggle per-pattern case sensitivity in the dialog
- Add or delete patterns at runtime
- Scroll with tail-follow mode
- Optional line wrapping

## Usage

```
Usage: logr [OPTIONS]

Options:
  -p, --patterns [<PATTERNS>...]  
  -i, --ignore-case               
  -h, --help                      Print help
  -V, --version                   Print version
```

```bash
dmesg | logr --patterns error,warning --ignore-case
```
