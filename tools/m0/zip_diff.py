"""Compare the parts of two OOXML/HWPX zip packages: which parts were added, removed or changed.
usage: zip_diff.py <original> <edited>  -> JSON on stdout"""
import hashlib, json, sys, zipfile

def parts(p):
    with zipfile.ZipFile(p) as z:
        return {i.filename: hashlib.sha1(z.read(i.filename)).hexdigest() for i in z.infolist() if not i.is_dir()}

a, b = parts(sys.argv[1]), parts(sys.argv[2])
print(json.dumps({
    "same": sorted(k for k in a if b.get(k) == a[k]),
    "changed": sorted(k for k in a if k in b and b[k] != a[k]),
    "removed": sorted(k for k in a if k not in b),
    "added": sorted(k for k in b if k not in a),
}, ensure_ascii=False))