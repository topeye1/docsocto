# Copy an HWPX, forcing settings.xml PrintMethod to 0 (one page per sheet).
# Prints the original PrintMethod value ("-" if absent).
import re, sys, zipfile

src, dst = sys.argv[1], sys.argv[2]
pat = re.compile(r'(name="PrintMethod" type="short">)(\d+)(<)')
orig = "-"
with zipfile.ZipFile(src) as zi, zipfile.ZipFile(dst, "w") as zo:
    for info in zi.infolist():
        data = zi.read(info)
        if info.filename == "settings.xml":
            s = data.decode("utf-8")
            m = pat.search(s)
            if m:
                orig = m.group(2)
                data = pat.sub(r"\g<1>0\3", s).encode("utf-8")
        zo.writestr(info, data)
print(orig)
