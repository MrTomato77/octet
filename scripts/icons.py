"""Cuts the Phosphor icon font down to the icons ui/index.html names (about 150 of its 1,530).

    pip install fonttools brotli
    python scripts/icons.py            after adding or renaming an icon
    python scripts/icons.py --check    fails if index.html names an icon the cut font lacks

The full font stays here in scripts/phosphor/, outside the ui folder that goes into the exe. A name is
kept when it appears anywhere in index.html as a whole word, so names chosen in code ("x" or
"dots-three") are kept too; a few common words ("copy", "check") cost a glyph each. So write a name
whole in index.html, never pieced together ("wifi-" + level): a pieced one is missed, even by --check.
"""
import re, sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
full, out = root / "scripts/phosphor", root / "ui/vendor/phosphor"
RULE = re.compile(r'\.ph\.ph-([a-z0-9-]+):before \{\s*content: "\\([0-9a-f]+)";\s*\}')

css = (full / "style.css").read_text(encoding="utf-8")
rules = dict(RULE.findall(css))
words = set(re.findall(r"[a-z0-9]+(?:-[a-z0-9]+)*", (root / "ui/index.html").read_text(encoding="utf-8")))
used = sorted(n for n in rules if n in words or "ph-" + n in words)

if "--check" in sys.argv:
    missing = [n for n in used if n not in dict(RULE.findall((out / "style.css").read_text(encoding="utf-8")))]
    sys.exit(f"not in the cut font, run scripts/icons.py: {missing}" if missing else 0)

from fontTools import subset

head = css[: css.index(".ph.ph-")]
(out / "style.css").write_text(head + "".join(f'.ph.ph-{n}:before {{\n  content: "\\{rules[n]}";\n}}\n' for n in used), encoding="utf-8", newline="\n")
opts = subset.Options()
opts.flavor, opts.layout_features = "woff2", []   # icons are picked by class, never by ligature
font = subset.load_font(str(full / "Phosphor.woff2"), opts)
cut = subset.Subsetter(opts)
cut.populate(unicodes=[int(rules[n], 16) for n in used])
cut.subset(font)
subset.save_font(font, str(out / "Phosphor.woff2"), opts)
print(f"{len(used)} of {len(rules)} icons")
