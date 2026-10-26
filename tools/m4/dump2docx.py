# M4-H2 시험판(결정 #33): Edge 덤프(html_dump) → WordCraft 명령 목록(octodocs run --script).
# 1차 방식은 "자리 고정": 꾸민 상자(배경·테두리·그라데이션)는 도형, 글이 든 블록은 투명 글상자로
# Edge가 그린 자리에 그대로 놓는다. 표지처럼 한 쪽에 짜인 화면용이며, 여러 쪽 본문은 흐름 방식을 따로 둔다.
# usage: python -I dump2docx.py <dump.json> <commands.json>
import json, re, sys

PX = 0.75  # CSS px → pt
INSET_X, INSET_Y = 7.2, 3.6  # WordCraft 글상자 기본 안쪽 여백(pt)


def rgba(s):
    m = re.match(r"rgba?\(([^)]*)\)", s or "")
    if not m:
        return None
    v = [float(x) for x in m.group(1).replace("/", ",").split(",")]
    a = v[3] if len(v) > 3 else 1.0
    return "%02X%02X%02X" % tuple(int(round(c)) for c in v[:3]), a


def gradient(s):
    if not s or "linear-gradient" not in s:
        return None
    body = s[s.index("(") + 1 : s.rindex(")")]
    angle = 180.0
    m = re.match(r"\s*(-?[\d.]+)deg\s*,", body)
    if m:
        angle = float(m.group(1))
    stops = []
    for c, pos in re.findall(r"(rgba?\([^)]*\))\s*([\d.]+%)?", body):
        col, a = rgba(c)
        stops.append([col, a, float(pos[:-1]) if pos else None])
    n = len(stops)
    out = []
    for i, (col, a, pos) in enumerate(stops):
        p = pos if pos is not None else (100.0 * i / max(1, n - 1))
        out.append({"pos": p, "color": col, "transparency": round((1 - a) * 100)})
    # CSS 각도(0=위로, 180=아래로) → Word 각도(0=왼→오, 90=위→아래)
    return {"stops": out, "type": "linear", "angle": (angle - 90) % 360}


def px(v):
    return float(v.rstrip("px")) if v and v.endswith("px") else 0.0


def decorated(n):
    st = n["style"]
    bg = rgba(st.get("background-color"))
    has_bg = bg and bg[1] > 0.001
    return has_bg or gradient(st.get("background-image")) or bool(sides(n))


def sides(n):
    """보이는 테두리 면: {면: (색, 두께pt)}"""
    st, out = n["style"], {}
    for s in ("top", "right", "bottom", "left"):
        w = px(st.get(f"border-{s}-width"))
        if w > 0 and st.get(f"border-{s}-style") not in ("none", "hidden"):
            c = rgba(st.get(f"border-{s}-color"))
            if c and c[1] > 0.001:
                out[s] = (c[0], w * PX)
    return out


def text_bounds(n):
    """블록이 바로 품은 글줄들의 바깥 상자(px)"""
    xs = [l["box"] for k in n["kids"] if k["tag"] == "#text" for l in k["lines"]]
    for k in n["kids"]:
        if k["tag"] not in ("#text", "br") and not is_block(k):
            b = text_bounds(k)
            if b:
                xs.append(b)
    if not xs:
        return None
    x0, y0 = min(b["x"] for b in xs), min(b["y"] for b in xs)
    x1, y1 = max(b["x"] + b["w"] for b in xs), max(b["y"] + b["h"] for b in xs)
    return {"x": x0, "y": y0, "w": x1 - x0, "h": y1 - y0}


def runs_of(n, inherit=None):
    """블록 안 글줄을 (줄 바꿈 포함) 서식 조각으로 편다."""
    out = []
    for k in n["kids"]:
        if k["tag"] == "#text":
            for i, l in enumerate(k["lines"]):
                if i > 0:
                    out.append(("\n", None))
                out.append((l["text"], n))
        elif k["tag"] == "br":
            out.append(("\n", None))
        elif not is_block(k):
            out += runs_of(k)
    return out


def is_block(n):
    return n["style"].get("display") not in ("inline", "inline-block") or decorated(n) or n["tag"] in ("table", "tr", "td", "th")


class Gen:
    def __init__(self):
        self.cmds = []
        self.objs = 0
        self.ops = []  # 개체별 명령 묶음 — 나중에 거꾸로 낸다

    def c(self, cmd, **p):
        self.cur.append({"command": cmd, "params": p})

    def begin(self):
        self.cur = []
        self.ops.append(self.cur)

    def finish(self):
        # 새 개체가 늘 본문 맨 앞에 들어가 먼저 그려지므로(아래 깔림), 바깥 상자부터 그려지게 거꾸로 낸다
        for op in reversed(self.ops):
            self.cmds += op

    def shape(self, n):
        st, b = n["style"], n["box"]
        rad = px(st.get("border-top-left-radius"))
        self.begin()
        self.c("caret.set", pos={"story": "body", "path": [0], "off": 0})
        self.c("insert.shape", kind="roundedRectangle" if rad > 0.5 else "rectangle", width=b["w"] * PX, height=b["h"] * PX)
        self.select_last()
        self.c("arrange.wrap", wrap="behindText")
        self.c("arrange.position", x=b["x"] * PX, y=b["y"] * PX)
        g = gradient(st.get("background-image"))
        bg = rgba(st.get("background-color"))
        if g:
            self.c("shape.fill", gradient=g)
        elif bg and bg[1] > 0.001:
            self.c("shape.fill", color=bg[0], transparency=round((1 - bg[1]) * 100))
        else:
            self.c("shape.fill", color=None)
        sd = sides(n)
        if len(sd) == 4 and len(set(sd.values())) == 1:
            col, w = next(iter(sd.values()))
            self.c("shape.outline", color=col, width=w)
        else:
            self.c("shape.outline", color=None)
            for s, (col, w) in sd.items():  # 한 면만 있는 테두리는 선으로
                x, y, ww, hh = b["x"] * PX, b["y"] * PX, b["w"] * PX, b["h"] * PX
                seg = {"top": (x, y, ww, 0), "bottom": (x, y + hh, ww, 0), "left": (x, y, 0, hh), "right": (x + ww, y, 0, hh)}[s]
                self.begin()
                self.c("caret.set", pos={"story": "body", "path": [0], "off": 0})
                self.c("insert.shape", kind="line", width=max(seg[2], 0.1), height=max(seg[3], 0.1), stroke=col)
                self.select_last()
                self.c("arrange.wrap", wrap="behindText")
                self.c("arrange.position", x=seg[0], y=seg[1])
                self.c("shape.outline", color=col, width=w)

    def select_last(self):
        # 새 개체는 본문 맨 앞(0번 자리)에 들어가 늘 첫 번째 개체다
        self.c("select.objects", index=0)
        self.objs += 1

    def textbox(self, n, runs):
        st, b = n["style"], n["box"]
        self.begin()
        self.c("caret.set", pos={"story": "body", "path": [0], "off": 0})
        t = text_bounds(n) or b
        pl, pr = px(st.get("padding-left")), px(st.get("padding-right"))
        cw = max(b["w"] - pl - pr, t["w"])  # 글이 차지할 수 있는 폭(안쪽 여백 뺀 상자 폭)
        cx = b["x"] + pl if st.get("text-align") in ("center", "right", "end") else t["x"]
        self.c("insert.textBox", text="", width=cw * PX * 1.10 + 2 * INSET_X, height=t["h"] * PX * 1.3 + 2 * INSET_Y)
        align = {"center": "center", "right": "right", "end": "right"}.get(st.get("text-align"), "left")
        if align != "left":
            self.c("para.align", value=align)
        lh = st.get("line-height")
        if lh and lh.endswith("px"):
            self.c("para.lineSpacing", exactly=px(lh) * PX)
        self.c("para.spacing", before=0, after=0)
        for text, src in runs:
            if src is None:
                self.c("text.lineBreak")
                continue
            s = src["style"]
            self.c("format.font", name="맑은 고딕", eastAsia="맑은 고딕")
            self.c("format.size", size=round(px(s.get("font-size")) * PX * 2) / 2)
            col = rgba(s.get("color"))
            self.c("format.color", color=col[0] if col else "000000")
            self.c("format.bold", value=int(s.get("font-weight", "400")) >= 600)
            self.c("text.insert", text=text)
        self.select_last()
        self.c("arrange.wrap", wrap="inFrontOfText")
        self.c("arrange.position", x=cx * PX - INSET_X, y=t["y"] * PX - INSET_Y)
        self.c("shape.fill", color=None)
        self.c("shape.outline", color=None)

    def walk(self, n):
        if n["tag"] == "#text":
            return
        if decorated(n):
            self.shape(n)
        # 글을 바로 품은 블록은 글상자 하나로(안쪽 인라인 조각 포함)
        if any(k["tag"] == "#text" for k in n["kids"]):
            runs = runs_of(n)
            if any(t.strip() for t, _ in runs):
                self.textbox(n, runs)
            for k in n["kids"]:
                if k["tag"] != "#text" and is_block(k):
                    self.walk(k)
            return
        for k in n["kids"]:
            self.walk(k)


def main(src, dst):
    d = json.load(open(src, encoding="utf-8"))
    g = Gen()
    g.cmds = [{"command": "layout.size", "params": {"name": "A4"}},
              {"command": "layout.margins", "params": {"top": 0, "bottom": 0, "left": 0, "right": 0}}]
    g.walk(d["root"])
    g.finish()
    json.dump({"commands": g.cmds}, open(dst, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    print(f"commands: {len(g.cmds)}, objects: {g.objs}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
