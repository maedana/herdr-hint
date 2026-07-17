# herdr-hint

Vimium-style hint labels for [Herdr](https://herdr.dev). Press a key to see all tabs and agents with labels, then press the label to jump.

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

- Tabs are grouped by workspace and displayed in two columns
- Agents show `repo:branch`, status, and terminal title (dimmed)
- With 26 or fewer items, labels are single-char (`a`-`z`)
- With more than 26 items, all labels switch to double-char (`aa`-`zz`) — press two keys to jump
- `Ctrl+D` / `Ctrl+U` to scroll half a page down/up when items overflow
- `Esc` to cancel

## Local development

```bash
herdr plugin link /path/to/herdr-hint
```

With `plugin link`, the plugin uses `cargo run` for development. Edit `herdr-plugin.toml` to use `["cargo", "run", "--quiet"]` as the pane command during development.
