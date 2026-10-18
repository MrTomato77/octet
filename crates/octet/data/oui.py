"""Builds oui.txt (who made a network card, by the first 3 bytes of its MAC) from the IEEE list.

    curl -A "Mozilla/5.0" -o oui.csv https://standards-oui.ieee.org/oui/oui.csv
    python oui.py oui.csv > oui.txt

Only MA-L blocks (the first 24 bits); the smaller MA-M/MA-S blocks are left out.
"""
import csv, re, sys

# Legal-form tails that only add length: "Hangzhou Hikvision Digital Technology Co.,Ltd." -> "Hangzhou Hikvision Digital Technology"
TAIL = re.compile(r"[\s,.]*\b(co\.?,?\s*ltd|co\.?\s*limited|company\s+limited|corporation|corp|incorporated|inc|ltd|limited|llc|gmbh|"
                  r"s\.?a\.?s?|s\.?r\.?l|b\.?v|a\.?g|ag|oy|ab|as|plc|pty|pte|kk|k\.k|co|company|technologies|technology|tech)\.?\s*$", re.I)

def short(name):
    name = " ".join(name.split()).strip(" ,.")
    for _ in range(4):
        cut = TAIL.sub("", name).strip(" ,.")
        if not cut or cut == name:
            break
        name = cut
    # ALL-CAPS names read as shouting next to the rest: REALTEK SEMICONDUCTOR -> Realtek Semiconductor (short acronyms stay).
    if name.isupper() and len(name) > 5:
        name = " ".join("-".join(p if len(p) <= 3 else p.capitalize() for p in w.split("-")) for w in name.split())
    return name

rows = {}
with open(sys.argv[1], encoding="utf-8") as f:
    for r in csv.DictReader(f):
        if r["Registry"] == "MA-L" and r["Organization Name"].strip():
            rows[r["Assignment"].upper()] = short(r["Organization Name"])
out = sys.stdout
out.reconfigure(encoding="utf-8", newline="\n")
for k in sorted(rows):
    out.write(f"{k}\t{rows[k]}\n")
