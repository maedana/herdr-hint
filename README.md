# herdr-hint

Vimium-style hint labels for [Herdr](https://herdr.dev). Press a key to see all tabs and agents with `a`-`z` labels, then press the label to jump.

## Install

Requires the Rust toolchain (`cargo`).

```bash
herdr plugin install maedana/herdr-hint
```

## Keybinding

Add to `~/.config/herdr/config.toml`:

```toml
[[keys.command]]
key = "prefix+f"
type = "shell"
command = "herdr plugin pane open --plugin maedana.hint --entrypoint jump"
description = "hint jump"
```

Then reload:

```bash
herdr server reload-config
```

## Usage

Press `prefix+f` to open the hint popup.

- Tabs are grouped by workspace
- Agents show `repo:branch` and status
- Press a label key (`a`-`z`) to jump
- Press `Esc` to cancel

## Local development

```bash
herdr plugin link /path/to/herdr-hint
```

With `plugin link`, the plugin uses `cargo run` for development. Edit `herdr-plugin.toml` to use `["cargo", "run", "--quiet"]` as the pane command during development.
