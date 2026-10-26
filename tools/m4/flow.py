# M4-H5 흐름 방식(결정 #33): Edge 덤프(html_dump) → 문단·표 블록 → docx(WordCraft) / HWPX(ohwp) 명령 목록.
# 표지("자리 고정", dump2docx·dump2hwpx)와 달리 글이 흘러 쪽을 넘긴다 — 여러 쪽 본문용.
#   제목·문단 → 문단, <table> → 표(칸 색·테두리·합침), 가로로 늘어선 블록(flex·다단) → 테두리 표,
#   바탕·테두리 있는 상자 → 1칸 표, 인라인 꼬리표(바탕 있는 inline-block) → 글자 음영.
# usage: python -I flow.py <dump.json> <docx|hwpx> <commands.json> [--page "A4;top;right;bottom;left(mm)"]
import json, re, sys, os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from dump2docx import PX, rgba, px  # noqa: E402

FONT = "맑은 고딕"
MM = 72 / 25.4  # mm → pt
A4 = (595.3, 841.9)


# ---------------------------------------------------------------- 덤프 읽기

def bg(n):
    c = rgba(n["style"].get("background-color"))
    return c[0] if c and c[1] > 0.001 else None


def side(n, s):
    st = n["style"]
    w = px(st.get(f"border-{s}-width"))
    if w <= 0 or st.get(f"border-{s}-style") in ("none", "hidden"):
        return None
    c = rgba(st.get(f"border-{s}-color"))
    return (c[0], round(w * PX, 2)) if c and c[1] > 0.001 else None


def box_border(n):
    """네 면이 같은 테두리면 (색, pt)"""
    s = [side(n, k) for k in ("top", "right", "bottom", "left")]
    return s[0] if s[0] and all(x == s[0] for x in s) else None


def elems(n):
    return [k for k in n["kids"] if k["tag"] != "#text"]


def is_inline(n):
    return n["style"].get("display") in ("inline", "inline-block") and n["tag"] not in ("table", "tr", "td", "th")


def has_text(n):
    return any(k["tag"] == "#text" for k in n["kids"]) or bool(n.get("before"))


def run_style(n, shade=None):
    s = n["style"]
    col = rgba(s.get("color"))
    return {"size": round(px(s.get("font-size")) * PX * 2) / 2, "bold": int(s.get("font-weight", "400")) >= 600,
            "italic": s.get("font-style") == "italic", "color": col[0] if col else "000000", "shade": shade}


def runs_of(n, shade=None):
    """인라인 내용 → [(글자, 서식)] — 줄은 Edge가 나눈 것이므로 이어 붙여 다시 흘린다"""
    out = []
    if n.get("before"):
        out.append((n["before"], run_style(n, shade)))
    for k in n["kids"]:
        if k["tag"] == "#text":
            t = "".join(l["text"] for l in k["lines"])
            if t:
                out.append((t, run_style(n, shade)))
        elif k["tag"] == "br":
            out.append(("\n", None))
        elif is_inline(k):
            out += runs_of(k, bg(k) or shade)
    return out


def tidy(runs):
    """HTML 공백 규칙: 연속 공백 하나로, 문단 앞뒤 공백 없앰"""
    out = []
    for t, s in runs:
        if s is None:
            out.append((t, s))
            continue
        t = re.sub(r"\s+", " ", t)
        if out and out[-1][1] is not None and out[-1][0].endswith(" ") and t.startswith(" "):
            t = t[1:]
        if t:
            out.append((t, s))
    while out and out[0][1] is not None and out[0][0].startswith(" "):
        t = out[0][0].lstrip()
        out[0:1] = [(t, out[0][1])] if t else []
    while out and out[-1][1] is not None and out[-1][0].endswith(" "):
        t = out[-1][0].rstrip()
        out[-1:] = [(t, out[-1][1])] if t else []
    return out


# ---------------------------------------------------------------- 블록 만들기

def para(n, runs, top=None, bottom=None):
    st, b = n["style"], n["box"]
    lh = st.get("line-height", "")
    bar = side(n, "left") if not box_border(n) and not bg(n) else None
    return {"kind": "para", "runs": runs, "align": {"center": "center", "right": "right", "end": "right", "justify": "justify"}.get(st.get("text-align"), "left"),
            "line": px(lh) * PX if lh.endswith("px") else None, "size": round(px(st.get("font-size")) * PX * 2) / 2,
            "bar": bar, "pad_left": px(st.get("padding-left")) * PX if bar else 0,
            "top": b["y"] if top is None else top, "bottom": b["y"] + b["h"] if bottom is None else bottom}


def page_break_after(n):
    st = n["style"]
    return "page" in (st.get("break-after", ""), st.get("page-break-after", "")) or st.get("break-after") == "page"


def flow(n, nested=False):
    """블록 n 의 내용 → [문단|표|쪽나눔]. nested 면(칸 안) 표·격자를 만들지 않고 문단으로만 편다."""
    if not nested:
        if n["tag"] == "table":
            return [html_table(n)]
        g = grid(n)
        if g:
            return [g]
        if (bg(n) or box_border(n)) and (has_text(n) or elems(n)) and n["tag"] not in ("body", "section"):
            return [box_table(n)]
    out, cur = [], []

    def flush():
        r = tidy(cur)
        if any(t.strip() for t, s in r if s):
            out.append(para(n, r))
        cur.clear()

    if n.get("before"):
        cur.append((n["before"], run_style(n)))
    for k in n["kids"]:
        if k["tag"] == "#text":
            cur.extend(runs_of({"kids": [k], "style": n["style"]}))
        elif k["tag"] == "br":
            cur.append(("\n", None))
        elif is_inline(k):
            cur.extend(runs_of(k, bg(k)))
        else:
            flush()
            out += flow(k, nested)
    flush()
    # 글자만 든 블록은 문단 자리를 블록 상자로(첫 줄 위 여백 포함)
    if len(out) == 1 and out[0]["kind"] == "para" and not elems(n):
        out[0]["top"], out[0]["bottom"] = n["box"]["y"], n["box"]["y"] + n["box"]["h"]
    if not nested and page_break_after(n) and out:
        out.append({"kind": "break"})
    return out


def cell_paras(n):
    return [b for b in flow(n, nested=True) if b["kind"] == "para"]


def cell_of(n, fill=None):
    st = n["style"]
    return {"paras": cell_paras(n), "bg": bg(n) or fill,
            "valign": {"middle": "center", "bottom": "bottom"}.get(st.get("vertical-align"), "top"),
            "pad": [px(st.get(f"padding-{s}")) * PX for s in ("top", "right", "bottom", "left")],
            "span": (n.get("rowspan", 1), n.get("colspan", 1)), "h": n["box"]["h"] * PX}


def bounds(xs, tol=1.5):
    out = []
    for x in sorted(xs):
        if not out or x - out[-1] > tol:
            out.append(x)
    return out


def col_at(bs, x):
    return min(range(len(bs)), key=lambda i: abs(bs[i] - x))


def html_table(n):
    rows = []
    for g in elems(n):
        rows += [r for r in ([g] if g["tag"] == "tr" else elems(g)) if r["tag"] == "tr"]
    cells = [(ri, c) for ri, r in enumerate(rows) for c in elems(r) if c["tag"] in ("td", "th")]
    bs = bounds([c["box"]["x"] for _, c in cells] + [c["box"]["x"] + c["box"]["w"] for _, c in cells])
    grid_cells = []
    for ri, c in cells:
        c0, c1 = col_at(bs, c["box"]["x"]), col_at(bs, c["box"]["x"] + c["box"]["w"])
        cell = cell_of(c)
        cell["at"] = (ri, c0)
        cell["span"] = (c.get("rowspan", 1), max(1, c1 - c0))
        grid_cells.append(cell)
    # 테두리는 본문 칸(td) 것을 표 전체에 — Word 쪽이 표 단위로만 받는다
    tds = [c for _, c in cells if c["tag"] == "td"] or [c for _, c in cells]
    return {"kind": "table", "rows": len(rows), "cols": len(bs) - 1, "widths": [(b - a) * PX for a, b in zip(bs, bs[1:])],
            "cells": grid_cells, "border": box_border(tds[0]) if tds else None, "inner": True,
            "top": n["box"]["y"], "bottom": n["box"]["y"] + n["box"]["h"]}


def grid(n):
    """가로로 늘어선 블록(flex 한 줄, 다단 목록)을 표로. 아니면 None."""
    ks = [k for k in elems(n) if not is_inline(k) or n["style"].get("display") == "flex"]
    if len(ks) < 2 or n["tag"] in ("table", "tr"):
        return None
    xs = bounds([k["box"]["x"] for k in ks], 2)
    if len(xs) < 2:
        return None
    b, fill = n["box"], bg(n)
    one_row = all(k["box"]["y"] < ks[0]["box"]["y"] + ks[0]["box"]["h"] for k in ks)
    if one_row:
        ks.sort(key=lambda k: k["box"]["x"])
        # 칸 경계는 이웃 블록 사이 빈 자리의 가운데 — Word·한글 글자가 Edge보다 조금 넓어도 접히지 않게
        edges = [b["x"]] + [(p["box"]["x"] + p["box"]["w"] + q["box"]["x"]) / 2 for p, q in zip(ks, ks[1:])] + [b["x"] + b["w"]]
        stacks = [elems(k) for k in ks]
        k0 = len(stacks[0])
        # 카드처럼 테두리 상자마다 같은 수의 블록이 위아래로 쌓였으면 그 수만큼 행으로 나눈다
        if k0 >= 2 and all(len(s) == k0 for s in stacks) and all(box_border(k) for k in ks):
            cells = [dict(cell_of(s[r]), at=(r, c), span=(1, 1)) for c, s in enumerate(stacks) for r in range(k0)]
            rows, border = k0, box_border(ks[0])
        else:
            # 바탕 있는 글자 조각(꼬리표)은 칸 색이 아니라 글자 음영으로
            cells = []
            for c, k in enumerate(ks):
                cell = cell_of(k, fill)
                if bg(k) and not elems(k):
                    cell["bg"] = fill
                    for p in cell["paras"]:
                        p["runs"] = [(t, dict(s, shade=bg(k)) if s else s) for t, s in p["runs"]]
                cell.update(at=(0, c), span=(1, 1), h=b["h"] * PX)
                if c == len(ks) - 1 and n["style"].get("display") == "flex" and k["box"]["x"] + k["box"]["w"] > b["x"] + b["w"] - 20:
                    for p in cell["paras"]:
                        p["align"] = "right"  # margin-left:auto 로 오른쪽 끝에 붙은 조각
                cells.append(cell)
            rows, border = 1, box_border(n)
        pad = [px(n["style"].get(f"padding-{s}")) * PX for s in ("top", "right", "bottom", "left")]
        for cl in cells:
            if fill and not any(cl["pad"]):
                cl["pad"] = [pad[0], 2, pad[2], 2]
    else:
        cols = [sorted([k for k in ks if abs(k["box"]["x"] - x) <= 2], key=lambda k: k["box"]["y"]) for x in xs]
        rows = max(len(c) for c in cols)
        edges = [b["x"]] + xs[1:] + [b["x"] + b["w"]]
        cells = [dict(cell_of(k), at=(r, c), span=(1, 1)) for c, col in enumerate(cols) for r, k in enumerate(col)]
        border = None
    return {"kind": "table", "rows": rows, "cols": len(edges) - 1, "widths": [(q - p) * PX for p, q in zip(edges, edges[1:])],
            "cells": cells, "border": border, "inner": border is not None and not one_row or bool(border and rows > 1),
            "fill": fill, "top": b["y"], "bottom": b["y"] + b["h"]}


def box_table(n):
    b = n["box"]
    return {"kind": "table", "rows": 1, "cols": 1, "widths": [b["w"] * PX], "cells": [dict(cell_of(n), at=(0, 0), span=(1, 1))],
            "border": box_border(n), "inner": False, "top": b["y"], "bottom": b["y"] + b["h"]}


def spacing(blocks):
    """블록 사이 빈 자리(Edge 상자 간격) → 앞 간격(pt). 쪽 나눔 뒤 첫 블록은 0."""
    prev = None
    for bl in blocks:
        if bl["kind"] == "break":
            prev = None
            continue
        bl["before"] = max(0.0, (bl["top"] - prev) * PX) if prev is not None else 0.0
        prev = bl["bottom"]
    return blocks


# ---------------------------------------------------------------- docx(WordCraft)

class Docx:
    def __init__(self, page):
        self.cmds = []
        self.blk = 0
        self.tables = 0
        self.prev = None
        size, top, right, bottom, left = page
        self.c("layout.size", name="A4")
        self.c("layout.margins", top=top, bottom=bottom, left=left, right=right)
        self.c("caret.set", pos=self.pos([0]))

    def c(self, cmd, **p):
        self.cmds.append({"command": cmd, "params": p})

    @staticmethod
    def pos(path, off=0):
        return {"story": "body", "path": path, "off": off}

    def para_fmt(self, p, before, in_cell=False):
        self.c("para.align", value=p["align"])
        if p["line"]:
            self.c("para.lineSpacing", exactly=p["line"])
        self.c("para.spacing", before=round(before, 1), after=0)
        self.c("para.indents", left=p["pad_left"], right=0, firstLine=0)
        # 한글·숫자/영문 사이 자동 띄우기 끄기 — Edge 에는 없는 빈칸이 생겼다("8 가지")
        self.c("para.asianTypography", autoSpaceDE=False, autoSpaceDN=False)
        if p["bar"]:
            col, w = p["bar"]
            self.c("para.borders", sides={"left": {"style": "single", "width": w, "color": col}})
        elif not in_cell:
            self.c("para.borders", kind="none")

    def runs(self, runs):
        for t, s in runs:
            if s is None:
                self.c("text.lineBreak")
                continue
            self.c("format.font", name=FONT, eastAsia=FONT)
            self.c("format.size", size=s["size"])
            self.c("format.color", color=s["color"])
            self.c("format.bold", value=s["bold"])
            self.c("format.italic", value=s["italic"])
            self.c("format.shading", color=s["shade"])
            self.c("text.insert", text=t)

    def gap(self, pt):
        """표 둘이 붙으면 Word가 한 표로 합친다 — 그 사이 간격을 빈 문단 하나로"""
        self.c("para.lineSpacing", exactly=max(1.0, round(pt, 1)))
        self.c("para.spacing", before=0, after=0)
        self.c("para.borders", kind="none")
        self.c("format.size", size=1)
        self.c("text.newParagraph")
        self.blk += 1

    def block(self, bl, page_break):
        if bl["kind"] == "para":
            if page_break:
                self.c("para.pageBreakBefore", value=True)
            self.para_fmt(bl, bl["before"])
            self.runs(bl["runs"])
            self.c("text.newParagraph")
            self.c("para.pageBreakBefore", value=False)
            self.blk += 1
        else:
            before = bl["before"]
            if page_break:
                self.c("para.pageBreakBefore", value=True)
                self.gap(max(1.0, before))
                self.c("para.pageBreakBefore", value=False)
            elif self.prev == "table" or before > 1:
                self.gap(before)
            self.table(bl)
        self.prev = bl["kind"]

    def table(self, t):
        b = self.blk
        self.c("insert.table", rows=t["rows"], cols=t["cols"])
        cell0 = lambda r, c: self.pos([b, r, c, 0])  # noqa: E731
        self.c("caret.set", pos=cell0(0, 0))
        if t["border"]:
            col, w = t["border"]
            if not t["inner"]:
                self.c("table.borders", kind="none")
            self.c("table.borders", kind="all" if t["inner"] else "outside", width=w, color=col)
        else:
            self.c("table.borders", kind="none")
        pad = t["cells"][0]["pad"]
        self.c("table.cellMargins", top=pad[0], right=pad[1], bottom=pad[2], left=pad[3])
        for c, w in enumerate(t["widths"]):
            self.c("caret.set", pos=cell0(0, c))
            self.c("table.columnWidth", width=round(w, 2))
        # 가로 합침 — 행마다 오른쪽부터 합쳐야 앞 칸 번호가 그대로다(세로 합침은 아직 없음)
        idx = {}
        for r in range(t["rows"]):
            spans = sorted([cl for cl in t["cells"] if cl["at"][0] == r], key=lambda cl: cl["at"][1])
            for cl in reversed(spans):
                c0, n = cl["at"][1], cl["span"][1]
                if n > 1:  # 공통 명령(표 번호·칸 좌표) — octodocs 가 WordCraft 합치기로 옮긴다
                    self.c("table.merge", table=self.tables, startRow=r, startCol=c0, endRow=r, endCol=c0 + n - 1)
            for i, cl in enumerate(spans):
                idx[id(cl)] = (r, i)
        for cl in t["cells"]:
            r, i = idx[id(cl)]
            self.c("caret.set", pos=cell0(r, i))
            self.c("table.shading", color=cl["bg"])
            self.c("table.properties", valign=cl["valign"])
            for k, p in enumerate(cl["paras"] or [None]):
                if k:
                    self.c("text.newParagraph")
                if p is None:
                    continue
                self.para_fmt(p, 0 if k == 0 else max(0.0, (p["top"] - cl["paras"][k - 1]["bottom"]) * PX), in_cell=True)
                self.runs(p["runs"])
        self.blk = b + 1
        self.tables += 1
        self.c("caret.set", pos=self.pos([self.blk]))


# ---------------------------------------------------------------- HWPX(ohwp)

HWP_W = [0.1, 0.12, 0.15, 0.2, 0.25, 0.3, 0.4, 0.5, 0.6, 0.7, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0]  # 한글 선 굵기 표(mm)
HU = 100  # pt → HWPUNIT
SPACE_X = 0.5  # 한글이 HWPX 문단 위 간격을 엔진 값의 두 배로 그렸다(본문 2쪽 실측) — 단위 확인은 할 일
FIXED_X = 2  # 한글이 HWPX 고정 줄 간격을 엔진 값의 절반으로 읽는다(표지 실측) — rhwp 단위 확인은 할 일


def hwp_line(col, w_pt):
    mm = w_pt / MM
    return {"type": 1, "width": min(range(len(HWP_W)), key=lambda i: abs(HWP_W[i] - mm)), "color": "#" + col}


NO_LINE = {"type": 0, "width": 0, "color": "#000000"}


class Hwpx:
    def __init__(self, page):
        self.cmds = []
        self.cur = 0  # 다음 블록을 쓸 빈 문단
        self.tables = 0
        size, top, right, bottom, left = page
        self.c("page.setup", width=A4[0], height=A4[1], top=top, bottom=bottom, left=left, right=right)

    def c(self, cmd, **p):
        self.cmds.append({"command": cmd, "params": p})

    def char_props(self, s):
        p = {"fontName": FONT, "fontSize": int(round(s["size"] * 100)), "textColor": "#" + s["color"], "bold": s["bold"], "italic": s["italic"]}
        if s["shade"]:
            p["shadeColor"] = "#" + s["shade"]
        return p

    def para_props(self, p, before):
        q = {"alignment": p["align"], "spacingBefore": int(round(before * HU * SPACE_X)), "spacingAfter": 0,
             "marginLeft": int(round(p["pad_left"] * HU)), "indent": 0, "autoSpaceKrEn": False, "autoSpaceKrNum": False}
        if p["line"]:
            q.update(lineSpacingType="Fixed", lineSpacing=int(round(p["line"] * HU * FIXED_X)))
        return q

    def pieces(self, runs):
        """줄 바꿈(\\n)마다 문단을 나눈 [(글자, 서식)] 묶음"""
        out = [[]]
        for t, s in runs:
            if s is None:
                out.append([])
            else:
                out[-1].append((t, s))
        return out

    def block(self, bl, page_break):
        if bl["kind"] == "para" and bl["bar"]:
            bl = self.bar_table(bl)  # 왼쪽 막대 제목은 왼쪽 테두리만 있는 1칸 표로
        if bl["kind"] == "para":
            for i, runs in enumerate(self.pieces(bl["runs"])):
                if i:
                    self.c("para.append", section=0, para=self.cur, text="")
                    self.cur += 1
                off = 0
                for t, s in runs:
                    self.c("text.insert", section=0, para=self.cur, offset=off, text=t)
                    self.c("format.char", section=0, para=self.cur, start=off, end=off + len(t), props=self.char_props(s))
                    off += len(t)
                q = self.para_props(bl, bl["before"] if i == 0 else 0)
                q["pageBreakBefore"] = page_break and i == 0
                self.c("format.para", section=0, para=self.cur, props=q)
            self.c("para.append", section=0, para=self.cur, text="")
            self.cur += 1
        else:
            self.c("format.para", section=0, para=self.cur,
                   props={"spacingBefore": int(round(bl["before"] * HU * SPACE_X)), "spacingAfter": 0, "pageBreakBefore": page_break,
                          "marginLeft": 0, "indent": 0, "alignment": "left", "lineSpacingType": "Percent", "lineSpacing": 100})
            self.table(bl)

    def bar_table(self, p):
        col, w = p["bar"]
        cell = {"paras": [dict(p, bar=None, pad_left=0)], "bg": None, "valign": "center", "pad": [0, 0, 0, p["pad_left"] - w],
                "at": (0, 0), "span": (1, 1), "lines": {"left": (col, w)}, "h": (p["bottom"] - p["top"]) * PX}
        return {"kind": "table", "rows": 1, "cols": 1, "widths": [None], "cells": [cell], "border": None, "inner": False,
                "before": p["before"], "top": p["top"], "bottom": p["bottom"]}

    def table(self, t):
        ti = self.tables
        self.tables += 1
        self.c("table.create", section=0, para=self.cur, offset=0, rows=t["rows"], cols=t["cols"])
        if all(w is not None for w in t["widths"]):
            self.c("table.columnWidths", table=ti, widths=[int(round(w * HU)) for w in t["widths"]])
        # 글자처럼 취급 — 한글은 떠 있는 표의 자리를 저장된 표 높이로 잡아, 내용이 늘면 아래 글과 겹쳤다
        self.c("table.props", table=ti, props={"paddingLeft": 0, "paddingRight": 0, "paddingTop": 0, "paddingBottom": 0, "cellSpacing": 0, "treatAsChar": True})
        for cl in t["cells"]:
            (r, c), (rs, cs) = cl["at"], cl["span"]
            if rs > 1 or cs > 1:
                self.c("table.merge", table=ti, startRow=r, startCol=c, endRow=r + rs - 1, endCol=c + cs - 1)
        # 행 높이 — 엔진이 칸 높이를 여백만큼(약 1mm)으로 저장해 한글이 표 아래 글을 표 위에 겹쳐 놓았다.
        # Edge 가 그린 행 높이를 칸 높이로 넣는다(한글은 내용이 더 길면 늘린다).
        row_h = [0.0] * t["rows"]
        for cl in t["cells"]:
            if cl["span"][0] == 1:
                row_h[cl["at"][0]] = max(row_h[cl["at"][0]], cl.get("h", 0))
        for cl in t["cells"]:
            r, c = cl["at"]
            height = sum(row_h[r:r + cl["span"][0]])
            line = hwp_line(*t["border"]) if t["border"] else NO_LINE
            sides = {k: line for k in ("Left", "Right", "Top", "Bottom")}
            if t["border"] and not t["inner"]:  # 바깥 테두리만 — 안쪽 경계는 없앤다
                if r > 0:
                    sides["Top"] = NO_LINE
                if r + cl["span"][0] < t["rows"]:
                    sides["Bottom"] = NO_LINE
                if c > 0:
                    sides["Left"] = NO_LINE
                if c + cl["span"][1] < t["cols"]:
                    sides["Right"] = NO_LINE
            for k, v in cl.get("lines", {}).items():
                sides[k.capitalize()] = hwp_line(*v)
            props = {f"border{k}": v for k, v in sides.items()}
            props.update(fillType="solid" if cl["bg"] else "none", fillColor="#" + (cl["bg"] or "FFFFFF"))
            pt, pr, pb, pl = cl["pad"]
            props.update(paddingTop=int(round(pt * HU)), paddingRight=int(round(pr * HU)), paddingBottom=int(round(pb * HU)),
                         paddingLeft=int(round(pl * HU)), applyInnerMargin=True,
                         verticalAlign={"top": 0, "center": 1, "bottom": 2}[cl["valign"]])
            if height > 0:
                props["height"] = int(round(height * HU))
            self.c("table.cellProps", table=ti, row=r, col=c, props=props)
            k = 0
            for pi, p in enumerate(cl["paras"]):
                for j, runs in enumerate(self.pieces(p["runs"])):
                    if k:
                        self.c("table.cellSplitPara", table=ti, row=r, col=c, para=k - 1, offset=self._len)
                    off = 0
                    for txt, s in runs:
                        self.c("table.cellInsertText", table=ti, row=r, col=c, para=k, offset=off, text=txt)
                        self.c("table.cellCharFormat", table=ti, row=r, col=c, para=k, start=off, end=off + len(txt), props=self.char_props(s))
                        off += len(txt)
                    self._len = off
                    gap = 0 if (pi == 0 or j) else max(0.0, (p["top"] - cl["paras"][pi - 1]["bottom"]) * PX)
                    self.c("table.cellParaFormat", table=ti, row=r, col=c, para=k, props=self.para_props(p, gap))
                    k += 1
        self.cur += 1


# ---------------------------------------------------------------- 실행

def main(argv):
    src, fmt, dst = argv[:3]
    page = ("A4", 14 * MM, 15 * MM, 16 * MM, 15 * MM)  # 시험 원본 @page: 14mm 15mm 16mm
    if "--page" in argv:
        v = argv[argv.index("--page") + 1].split(";")
        page = (v[0],) + tuple(float(x) * MM for x in v[1:5])
    d = json.load(open(src, encoding="utf-8"))
    blocks = flow(d["root"])
    while blocks and blocks[-1]["kind"] == "break":  # 마지막 쪽 뒤 나눔은 빈 쪽만 만든다
        blocks.pop()
    blocks = spacing(blocks)
    g = Docx(page) if fmt == "docx" else Hwpx(page)
    brk = False
    for bl in blocks:
        if bl["kind"] == "break":
            brk = True
            continue
        g.block(bl, brk)
        brk = False
    if fmt == "hwpx":  # 다 채운 뒤 쪽 설정을 다시 걸어 엔진이 전체를 다시 조판하게 한다(표 높이·줄 자리 갱신)
        g.cmds.append(g.cmds[0])
    json.dump({"commands": g.cmds}, open(dst, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    kinds = [b["kind"] for b in blocks]
    print(f"{fmt}: blocks {len(blocks)} (para {kinds.count('para')}, table {kinds.count('table')}, break {kinds.count('break')}), commands {len(g.cmds)}")


if __name__ == "__main__":
    main(sys.argv[1:])
