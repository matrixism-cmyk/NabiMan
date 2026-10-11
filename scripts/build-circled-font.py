#!/usr/bin/env python3
"""Build the one-cell circled-digit font used by the web terminal.

Requires fontTools and a local DejaVu Sans TTF. DejaVu's license is copied
alongside the generated web font.
"""

from pathlib import Path
import sys

from fontTools import subset
from fontTools.ttLib import TTFont


SOURCE = Path(sys.argv[1]) if len(sys.argv) > 1 else Path('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf')
OUTPUT = Path(__file__).resolve().parents[1] / 'frontend/src/assets/circled-digits.woff'

font = TTFont(SOURCE)
codepoints = range(0x2460, 0x246A)
names = {font.getBestCmap()[codepoint] for codepoint in codepoints}
options = subset.Options()
options.hinting = False
subsetter = subset.Subsetter(options=options)
subsetter.populate(unicodes=codepoints)
subsetter.subset(font)

# The original glyph spans almost a whole em. xterm and tmux assign these
# ambiguous-width symbols one cell (about 0.6 em), so narrow the outlines.
advance = round(font['head'].unitsPerEm * 0.6)
for name in names:
    glyph = font['glyf'][name]
    for index, (x, y) in enumerate(glyph.coordinates):
        glyph.coordinates[index] = (round(x * 0.65), y)
    glyph.recalcBounds(font['glyf'])
    font['hmtx'][name] = (advance, round(font['hmtx'][name][1] * 0.65))

for record in font['name'].names:
    if record.nameID in (1, 4, 6):
        record.string = 'NabiMan Circled Digits'.encode(record.getEncoding())
    elif record.nameID == 2:
        record.string = 'Regular'.encode(record.getEncoding())

font['OS/2'].xAvgCharWidth = advance
font.flavor = 'woff'
OUTPUT.parent.mkdir(parents=True, exist_ok=True)
font.save(OUTPUT)
print(f'Created {OUTPUT}')
