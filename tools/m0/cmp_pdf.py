# Compare two PDF sets page by page: raw pixel diff, and diff after shifting
# page B by the (dx, dy) that best lines its ink up with page A.
# usage: cmp_pdf.py <dirA> <suffixA> <dirB> <suffixB> <out.json> ["name|side.png"]
#   pairs = every <dirA>/<name><suffixA>.pdf with a matching <dirB>/<name><suffixB>.pdf
#   An empty suffix may be '' or '-'; PowerShell 5.1 drops '' from native argv,
#   so suffixes are optional and arguments are told apart by what they are.
#   shift_*_mm = how far page B sits below/right of page A (negative = above/left).
import glob, json, os, sys
import fitz
import numpy as np
from PIL import Image, ImageDraw, ImageFont

DPI = 60
MAXSHIFT = 30  # px at DPI (about 12.7 mm)

def gray(page):
    pm = page.get_pixmap(dpi=DPI, colorspace=fitz.csGRAY)
    return np.frombuffer(pm.samples, np.uint8).reshape(pm.height, pm.width)

def best_shift(pa, pb):
    # 1-D cross-correlation of ink profiles, rows for dy and columns for dx
    def shift(a, b):
        a, b = a - a.mean(), b - b.mean()
        best, arg = None, 0
        for s in range(-MAXSHIFT, MAXSHIFT + 1):
            x = a[max(0, s):len(a) + min(0, s)]
            y = b[max(0, -s):len(b) + min(0, -s)]
            n = min(len(x), len(y))
            v = float((x[:n] * y[:n]).sum())
            if best is None or v > best:
                best, arg = v, s
        return arg
    ia, ib = (pa < 128).astype(float), (pb < 128).astype(float)
    return shift(ia.sum(1), ib.sum(1)), shift(ia.sum(0), ib.sum(0))

def roll(img, dy, dx):
    out = np.full_like(img, 255)
    h, w = img.shape
    out[max(0, dy):h + min(0, dy), max(0, dx):w + min(0, dx)] = \
        img[max(0, -dy):h + min(0, -dy), max(0, -dx):w + min(0, -dx)]
    return out

def ratio(a, b):
    h, w = min(a.shape[0], b.shape[0]), min(a.shape[1], b.shape[1])
    return float((np.abs(a[:h, :w].astype(int) - b[:h, :w].astype(int)) > 64).mean())

def parse(argv):
    pairs, outjson, side = [], None, None
    for a in argv:
        if "|" in a: side = a
        elif a.lower().endswith(".json"): outjson = a
        elif os.path.isdir(a): pairs.append([a, ""])
        elif pairs: pairs[-1][1] = "" if a == "-" else a
    (da, sa), (db, sb) = pairs
    return da, sa, db, sb, outjson, side

da, sa, db, sb, outjson, side = parse(sys.argv[1:])
res = []
for pa_path in sorted(glob.glob(os.path.join(da, "*" + sa + ".pdf"))):
    name = os.path.basename(pa_path)[: -len(sa + ".pdf")]
    pb_path = os.path.join(db, name + sb + ".pdf")
    if not os.path.exists(pb_path):
        continue
    A, B = fitz.open(pa_path), fitz.open(pb_path)
    raw, aligned, dys, dxs = [], [], [], []
    for pa, pb in zip(A, B):
        ga, gb = gray(pa), gray(pb)
        dy, dx = best_shift(ga, gb)
        raw.append(ratio(ga, gb))
        # shift() gives a[i+s] ~ b[i]: moving B by +s lines it up with A
        aligned.append(ratio(ga, roll(gb, dy, dx)))
        dys.append(dy); dxs.append(dx)
    mm = 25.4 / DPI
    r = {
        "doc": name, "pages_a": A.page_count, "pages_b": B.page_count,
        "raw_mean": round(np.mean(raw) * 100, 2),
        "aligned_mean": round(np.mean(aligned) * 100, 2),
        "aligned_max": round(max(aligned) * 100, 2),
        "worst_page": int(np.argmax(aligned)) + 1,
        "shift_dy_mm": round(-float(np.median(dys)) * mm, 1),
        "shift_dx_mm": round(-float(np.median(dxs)) * mm, 1),
        "pages_over_1pct": int(sum(v > 0.01 for v in aligned)),
    }
    res.append(r)
    print(json.dumps(r, ensure_ascii=False), flush=True)
json.dump(res, open(outjson, "w", encoding="utf-8"), ensure_ascii=False, indent=1)

if side:
    name, png = side.split("|")
    def rgb(p, i):
        pm = fitz.open(p)[i].get_pixmap(dpi=90)
        return Image.frombytes("RGB", (pm.width, pm.height), pm.samples)
    x = rgb(os.path.join(da, name + sa + ".pdf"), 0)
    y = rgb(os.path.join(db, name + sb + ".pdf"), 0)
    img = Image.new("RGB", (x.width + y.width + 30, max(x.height, y.height) + 40), "white")
    img.paste(x, (0, 40)); img.paste(y, (x.width + 30, 40))
    ft = ImageFont.truetype(r"C:\Windows\Fonts\malgun.ttf", 20)
    dr = ImageDraw.Draw(img)
    dr.text((10, 8), "A: " + da, font=ft, fill="black")
    dr.text((x.width + 40, 8), "B: " + db, font=ft, fill="black")
    img.save(png)
    print("saved", png)
