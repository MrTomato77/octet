# Contributing to Octet

Thanks for helping. Bugs and ideas are welcome as [issues](https://github.com/octet-app/octet/issues/new/choose), or from Octet itself: *Settings → General → Feedback*.

## Build and check

```bash
cargo build --release -p octet      # target/release/octet.exe
cargo test --release -p octet
python scripts/icons.py --check     # every Phosphor icon the UI names must be in the cut font
```

The UI is one file, `ui/index.html`. Open it in a browser for a demo with sample data and a *Scenario* picker; no build needed. On a Mac, `crates/octet/macos/bundle.sh` makes `Octet.app` and `Octet.dmg`. More in [docs/guide.md](docs/guide.md).

## Pull requests

- One change per pull request, with what it fixes or adds in the title.
- A UI change: a screenshot, in English and ไทย if it has text (both live in `ui/index.html`).
- A change that touches the network settings: say on which adapters and which OS you tried it.
- Words that happen to be icon names (for example `bridge` in a comment) count as used icons. If `icons.py --check` complains, reword or run `python scripts/icons.py`.

Security problems: see [SECURITY.md](SECURITY.md), not a public issue.
