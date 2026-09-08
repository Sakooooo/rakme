# rakme

A clone of Plan 9's [acme](http://acme.cat-v.org/) text editor, written in
Rust on top of `winit`, `pixels` and `tiny-skia`.

```sh
cargo run                    # opens the current directory
cargo run -- src/main.rs     # opens files
```

A monospace TrueType font is loaded from the system (Consolas, DejaVu Sans
Mono, Menlo, ...). Set `RAKME_FONT=/path/to/font.ttf` to choose one and
`RAKME_FONT_SIZE=16` to change the size.

## The mouse

Everything is driven by the three mouse buttons, as in acme.

| Action | Effect |
| --- | --- |
| Button 1 | Select text. Double-click selects a word, a line, or the text between matching brackets or quotes. |
| Button 2 | Execute the word or swept text as a command (built-in or shell). |
| Button 3 | Look: open the file named under the pointer (with an optional `:line`, `:$` or `:/text` address), or search for the next occurrence of the text. |
| 1 then 2 | Cut the selection. |
| 1 then 3 | Paste over the selection. |
| 1, 2, then 3 | Snarf (copy) the selection. |
| 2 then 1 | Execute the swept command with the current selection as its argument. |
| Wheel | Scroll the window under the pointer. |

Scrollbar: button 1 scrolls up, button 3 scrolls down, button 2 jumps.
The little square at the left of a window's tag turns dark blue when the
window is modified. Click it with button 1 to grow the window, drag it to
move the window, or click it with button 2 to fill the column. The square
in a column's tag grows or drags the column the same way.

## Commands

Run any of these by clicking them with button 2 in a tag or body.

- Row tag: `Newcol`, `Putall`, `Exit`
- Column tag: `New`, `Cut`, `Paste`, `Snarf`, `Sort`, `Zerox`, `Delcol`
- Window tag: `Del`, `Snarf`, `Undo`, `Redo`, `Put`, `Look`, plus `Get`
  on directory windows
- Also: `Delete` (close without saving), `Tab n`, `Font [size|path]`,
  `Send`, `Id`, `New name`, `Get name`, `Put name`, `Look text`

`Del` and `Get` on a modified window refuse once and report in `+Errors`;
run them again to confirm. Change the file name at the start of a tag and
`Put` to save under a new name.

Anything else is run through the shell in the window's directory, with its
output appended to that directory's `+Errors` window. Prefix a command with
`|` to pipe the selection through it and replace it with the output, `<` to
replace the selection with the output, or `>` to send the selection to its
input.

## The keyboard

Typing replaces the selection. `Backspace`, `Delete`, arrows, `Home`,
`End`, `PageUp` and `PageDown` do the usual; `Ctrl-A`/`Ctrl-E` go to the
start/end of the line, `Ctrl-U` deletes to the start of the line, `Ctrl-W`
deletes the previous word and `Esc` selects the text typed since the last
click.
