# xconv_od.ps1 요약: Octo Docs 변환 결과와 M0(한글 변환) 결과를 파일마다 나란히.
# usage: python -I xconv_sum.py <Out> <M0 xconv 폴더>
import json, os, sys

sys.stdout.reconfigure(encoding="utf-8")
out, base = sys.argv[1], sys.argv[2]


def load(p):
    try:
        return {r["doc"]: r for r in json.load(open(p, encoding="utf-8"))}
    except Exception:
        return {}


for d in ("h2d", "d2h"):
    od, m0 = load(os.path.join(out, f"cmp_{d}.json")), load(os.path.join(base, f"cmp_{d}.json"))
    if not od:
        continue
    same = 0
    print(f"== {d}  (쪽 수 원본/변환, 맞춘 뒤 차이%  |  M0 한글 변환)")
    for k in sorted(set(od) | set(m0)):
        a, b = od.get(k), m0.get(k)
        sa = f"{a['pages_a']}/{a['pages_b']} {a['aligned_mean']:5.1f}%" if a else "-"
        sb = f"{b['pages_a']}/{b['pages_b']} {b['aligned_mean']:5.1f}%" if b else "-"
        if a and a["pages_a"] == a["pages_b"]:
            same += 1
        print(f"RESULT {d} {k[:28]:28s} OD {sa:16s} | M0 {sb}")
    print(f"SUMMARY {d}: 변환 {len(od)}건, 쪽 수 같음 {same}건")
