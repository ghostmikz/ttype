# ttype

A typing test that lives in your terminal. English and Mongolian word lists,
live WPM, and a results screen with a speed-over-time graph.

```
  ttype    15s  30s  60s  120s  │  10w  25w  50w  100w  │  en  mn

              30   best 98 wpm

              come long open ask possible little say other on hand it can possible
              than know those child it general such in problem ask against help be
              place nation find much before other be hold no small a give possible on

                        tab restart  ·  ←→ mode  ·  ↑↓ language  ·  esc quit
```

## Install

```sh
cargo install --git https://github.com/ghostmikz/ttype
```

or from a clone: `cargo install --path .`

## Usage

```sh
ttype              # 30 second test, english
ttype -t 60        # 60 seconds
ttype -w 25        # 25 words
ttype -l mn        # mongolian (switch your keyboard layout first)
```

| key | action |
|---|---|
| `tab` | restart with new words |
| `←` `→` | cycle test mode |
| `↑` `↓` | switch language |
| `ctrl+backspace` / `ctrl+w` | delete word |
| `esc` | quit |

The timer starts on your first keystroke. A word you typed correctly is locked
in; backspace can only step back into a word you got wrong.

## Results

- **wpm**: characters of correctly typed words (plus their spaces) / 5 per minute
- **raw**: every character typed, right or wrong
- **accuracy**: correct keystrokes / all keystrokes
- **consistency**: how even your per-second speed was
- **characters**: correct / incorrect / extra / missed
- **missed keys**: which characters you fumbled most

Every run is appended to `~/.local/share/ttype/history.jsonl`, and your best
for the current mode + language is shown before each test.

## Development

```sh
cargo run -- -w 10
cargo test
cargo test preview -- --ignored --nocapture   # render screens as text
```
