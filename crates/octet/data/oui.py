"""Builds oui.txt (who made a network card, by the start of its MAC) from the IEEE lists.

    curl -A "Mozilla/5.0 ..." -o oui.csv   https://standards-oui.ieee.org/oui/oui.csv       (MA-L, 24 bits)
    curl -A "Mozilla/5.0 ..." -o mam.csv   https://standards-oui.ieee.org/oui28/mam.csv     (MA-M, 28 bits)
    curl -A "Mozilla/5.0 ..." -o oui36.csv https://standards-oui.ieee.org/oui36/oui36.csv   (MA-S, 36 bits)
    python oui.py oui.csv mam.csv oui36.csv > oui.txt

One line per block: the prefix in hex (6, 7 or 9 digits), a tab, the maker. A MAC is looked up longest
prefix first. MA-L blocks the IEEE split into MA-M/MA-S are listed under "IEEE Registration Authority",
which is not who made the device: they are left out, so only their sub-blocks answer. So are blocks
whose owner asked to stay unnamed ("Private").
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
    # All lower case reads as a typo: zte corporation -> ZTE, shenzhen foo -> Shenzhen Foo.
    if name.islower():
        name = name.upper() if len(name) <= 5 else name.title()
    return name

NOT_A_MAKER = {"ieee registration authority", "private"}
rows = {}
for path in sys.argv[1:]:
    with open(path, encoding="utf-8") as f:
        for r in csv.DictReader(f):
            name = r["Organization Name"].strip()
            if r["Registry"] in ("MA-L", "MA-M", "MA-S") and name and name.lower() not in NOT_A_MAKER:
                rows[r["Assignment"].upper()] = short(name)
out = sys.stdout
out.reconfigure(encoding="utf-8", newline="\n")
for k in sorted(rows):
    out.write(f"{k}\t{rows[k]}\n")
