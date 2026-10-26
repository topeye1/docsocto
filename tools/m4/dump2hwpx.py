# M4-H4 시험판(결정 #33): Edge 덤프(html_dump) → Octo Docs 한글 명령 목록(octodocs run --script).
# dump2docx 와 같은 "자리 고정" 방식: 꾸민 상자는 쪽 기준 도형(shape.add), 글이 든 블록은 테두리 없는 글상자(shape.text).
# usage: python -I dump2hwpx.py <dump.json> <commands.json>
import json, sys, os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from dump2docx import PX, rgba, gradient, px, decorated, sides, text_bounds, text_frame, runs_of, is_block  # noqa: E402

FONT = "맑은 고딕"
OUT_DIR = None  # 그러데이션 그림을 둘 폴더(main 에서 정함)


def hexrgb(h):
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def grad_color_at(g, frac):
    """그러데이션(dump2docx 형식, 위→아래 가정) frac(0~1) 자리 색"""
    st = sorted(g["stops"], key=lambda s: s["pos"])
    p = frac * 100
    for a, b in zip(st, st[1:]):
        if a["pos"] <= p <= b["pos"]:
            t = (p - a["pos"]) / max(1e-6, b["pos"] - a["pos"])
            ca, cb = hexrgb(a["color"]), hexrgb(b["color"])
            return tuple(round(ca[i] + (cb[i] - ca[i]) * t) for i in range(3))
    return hexrgb(st[-1]["color"] if p > st[-1]["pos"] else st[0]["color"])


def gradient_png(g, w_pt, h_pt, name):
    """한글은 그러데이션을 띠로 그려 PDF에 가는 줄이 남는다 — 그림으로 바꿔 넣는다"""
    from PIL import Image
    # 96dpi 화소 — 한글은 그림을 화소 크기 기준으로 다루므로 쪽 크기와 맞춘다
    W, H = max(1, round(w_pt * 96 / 72)), max(1, round(h_pt * 96 / 72))
    img = Image.new("RGB", (W, H))
    vertical = abs(((g["angle"]) % 180) - 90) < 45  # Word 각도 90 = 위→아래
    for i in range(H if vertical else W):
        c = grad_color_at(g, i / max(1, (H if vertical else W) - 1))
        if vertical:
            img.paste(c, (0, i, W, i + 1))
        else:
            img.paste(c, (i, 0, i + 1, H))
    path = os.path.join(OUT_DIR, name)
    img.save(path)
    return path


def fill_of(st):
    g = gradient(st.get("background-image"))
    if g:
        # dump2docx 의 각도는 Word 기준(0=왼→오, 90=위→아래) — 한글은 0=위→아래, 90=왼→오
        css = (g["angle"] + 90) % 360
        return {"gradient": {"angle": (180 - css) % 360,
                             "stops": [{"pos": s["pos"], "color": s["color"], "alpha": 1 - s["transparency"] / 100} for s in g["stops"]]}}
    bg = rgba(st.get("background-color"))
    if bg and bg[1] > 0.001:
        return {"color": bg[0], "alpha": bg[1]}
    return None


class Gen:
    def __init__(self):
        self.cmds = []
        self.under = []  # 바깥 상자들의 (상자, 채우기) — 반투명 색을 아래 색과 섞을 때 쓴다
        self.pics = 0

    def backdrop(self, b):
        """상자 b 가운데 아래에 깔린 색"""
        cy = b["y"] + b["h"] / 2
        for ob, f in reversed(self.under):
            if f.get("grad"):
                return grad_color_at(f["grad"], (cy - ob["y"]) / max(1, ob["h"]))
            if f.get("rgb"):
                return f["rgb"]
        return (255, 255, 255)

    def c(self, cmd, **p):
        self.cmds.append({"command": cmd, "params": p})

    def shape(self, n):
        st, b = n["style"], n["box"]
        x, y, w, h = b["x"] * PX, b["y"] * PX, b["w"] * PX, b["h"] * PX
        rad = px(st.get("border-top-left-radius")) * PX
        sd = sides(n)
        line = None
        if len(sd) == 4 and len(set(sd.values())) == 1:
            col, lw = next(iter(sd.values()))
            line = {"color": col, "width": lw}
        g = gradient(st.get("background-image"))
        bg = rgba(st.get("background-color"))
        fill = None
        if g:
            self.under.append((b, {"grad": g}))
            if rad <= 0.5 and line is None:
                self.pics += 1
                path = gradient_png(g, w, h, f"grad{self.pics}.png")
                self.c("picture.add", path=path, x=x, y=y, w=w, h=h, wrap="BehindText")
                return
            fill = fill_of(st)
        elif bg and bg[1] > 0.001:
            if bg[1] < 0.999:  # 반투명은 아래 색과 섞어 단색으로(한글은 반투명을 무늬로 찍었다)
                under = self.backdrop(b)
                top = hexrgb(bg[0])
                mix = tuple(round(top[i] * bg[1] + under[i] * (1 - bg[1])) for i in range(3))
                fill = {"color": "%02X%02X%02X" % mix, "alpha": 1}
            else:
                fill = {"color": bg[0], "alpha": 1}
            self.under.append((b, {"rgb": hexrgb(fill["color"])}))
        # 배경 그림만 글 뒤에 깔고, 나머지 꾸밈 도형은 그 위(글 앞)에 둔다 — 같은 "글 뒤"끼리는 그림이 도형을 덮었다
        p = {"kind": "rectangle", "x": x, "y": y, "w": w, "h": h, "wrap": "InFrontOfText", "line": line, "fill": fill}
        if rad > 0.5:
            p["round"] = int(min(50, round(rad / max(1, min(w, h)) * 100)))
        self.c("shape.add", **p)
        if line is None:
            for s, (col, lw) in sd.items():  # 한 면만 있는 테두리는 선으로
                seg = {"top": (x, y, w, 0), "bottom": (x, y + h, w, 0), "left": (x, y, 0, h), "right": (x + w, y, 0, h)}[s]
                self.c("shape.add", kind="line", x=seg[0], y=seg[1], w=max(seg[2], 0.1), h=max(seg[3], 0.1),
                       wrap="InFrontOfText", line={"color": col, "width": lw})

    def textbox(self, n, runs):
        st, b = n["style"], n["box"]
        t = text_bounds(n) or b
        pl, pr = px(st.get("padding-left")), px(st.get("padding-right"))
        fx, fw, _ = text_frame(n, b, t, 1.10)
        self.c("shape.add", kind="textbox", x=fx * PX, y=t["y"] * PX, w=fw * PX, h=t["h"] * PX * 1.3,
               wrap="InFrontOfText", line=None, fill=None, margin=[0, 0, 0, 0])
        para = {"alignment": {"center": "center", "right": "right", "end": "right"}.get(st.get("text-align"), "left")}
        lh = st.get("line-height")
        if lh and lh.endswith("px"):
            # 한글은 HWPX 고정 줄 간격 값을 엔진(rhwp)의 절반으로 읽는다(실측 35pt→17.5pt) — 두 배로 넣는다(rhwp 쪽 단위 확인은 할 일)
            para.update(lineSpacingType="Fixed", lineSpacing=int(round(px(lh) * PX * 100 * 2)))
        pending = ""
        for text, src in runs:
            if src is None:
                pending = "\n"
                continue
            s = src["style"]
            col = rgba(s.get("color"))
            props = {"fontName": FONT, "fontSize": int(round(px(s.get("font-size")) * PX * 100)),
                     "textColor": "#" + (col[0] if col else "000000"), "bold": int(s.get("font-weight", "400")) >= 600}
            self.c("shape.text", text=pending + text, props=props, paraProps=para)
            pending = ""

    def walk(self, n):
        if n["tag"] == "#text":
            return
        depth = len(self.under)
        try:
            self._walk(n)
        finally:
            del self.under[depth:]  # 이 상자의 바탕은 그 안쪽에만 깔린다

    def _walk(self, n):
        if decorated(n):
            self.shape(n)
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
    global OUT_DIR
    OUT_DIR = os.path.dirname(os.path.abspath(dst))
    d = json.load(open(src, encoding="utf-8"))
    g = Gen()
    g.walk(d["root"])
    json.dump({"commands": g.cmds}, open(dst, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    print(f"commands: {len(g.cmds)}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
