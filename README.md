# ttype

A typing test that lives in your terminal: live WPM, a caret that glides
between letters, a speed-over-time graph and a heatmap of the keys you miss.

```
  ttype    15s  30s  60s  120s  │  10w  25w  50w  100w

              30   best 98 wpm

              come long open ask possible little say other on hand it can possible
              than know those child it general such in problem ask against help be
              place nation find much before other be hold no small a give possible on

                             tab restart  ·  ←→ mode  ·  esc quit
```

## Install

```sh
cargo install --git https://github.com/ghostmikz/ttype
```

or from a clone: `cargo install --path .`

## Usage

```sh
ttype              # 30 second test
ttype -t 60        # 60 seconds
ttype -w 25        # 25 words
```

| key | action |
|---|---|
| `tab` | restart with new words |
| `←` `→` | cycle test mode |
| `ctrl+backspace` / `ctrl+w` | delete word |
| `h` | results: chart / heatmap / all-time heatmap |
| `esc` | quit |

The timer starts on your first keystroke. A word you typed correctly is locked
in; backspace can only step back into a word you got wrong.

The text column widens with your terminal (up to 110 columns) and the results
screen stretches to fill a large window. For bigger letters, zoom your
terminal's font (ctrl+shift+plus in most terminals).

## Results

- **wpm**: characters of correctly typed words (plus their spaces) / 5 per minute
- **raw**: every character typed, right or wrong
- **accuracy**: correct keystrokes / all keystrokes
- **consistency**: how even your per-second speed was
- **characters**: correct / incorrect / extra / missed
- **missed keys**: which characters you fumbled most

Press `h` on the results screen to cycle between the speed chart and two
keyboard heatmaps:

- **this test**: each key coloured by how many times you missed it
- **all time**: each key coloured by its error rate across every saved run
  (a key needs 20+ attempts before it's ranked, so one unlucky
  miss on a rare letter doesn't dominate)

A miss is charged to the key you *should* have pressed, including letters you
skipped by hitting space early. Extra characters past the end of a word don't
count against any key.

Every run is appended to `~/.local/share/ttype/history.jsonl`, and your best
for the current mode is shown before each test.

## Development

```sh
cargo run -- -w 10
cargo test
TTYPE_PREVIEW=/tmp/ttype.html TTYPE_PREVIEW_SIZE=120x40 \
  cargo test html_preview -- --ignored   # render screens to html, in colour
```
