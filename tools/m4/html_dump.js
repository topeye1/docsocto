// M4 (결정 #33): Edge가 그린 결과를 그대로 받는다 — 요소마다 최종 스타일·자리, 글줄마다 글자·자리.
// html_dump.ps1 이 원본 HTML 사본 끝에 이 스크립트를 붙여 Edge(headless --dump-dom)로 연다.
// 결과는 <pre id="octodocs-dump"> 안의 JSON. 단위는 CSS px(1px = 0.75pt).
(function () {
  const KEEP = ['display', 'position', 'color', 'background-color', 'background-image', 'font-family', 'font-size',
    'font-weight', 'font-style', 'text-decoration-line', 'text-align', 'line-height', 'letter-spacing',
    'border-top-width', 'border-top-style', 'border-top-color', 'border-right-width', 'border-right-style',
    'border-right-color', 'border-bottom-width', 'border-bottom-style', 'border-bottom-color', 'border-left-width',
    'border-left-style', 'border-left-color', 'border-top-left-radius', 'padding-top', 'padding-right',
    'padding-bottom', 'padding-left', 'opacity', 'vertical-align', 'break-before', 'break-after', 'page-break-before',
    'page-break-after', 'white-space'];
  const r = (v) => Math.round(v * 100) / 100;
  const box = (b) => ({ x: r(b.left + scrollX), y: r(b.top + scrollY), w: r(b.width), h: r(b.height) });
  let nextId = 0;
  function walk(el, parent) {
    const cs = getComputedStyle(el);
    if (cs.display === 'none' || ['SCRIPT', 'STYLE', 'HEAD', 'META', 'TITLE', 'LINK'].includes(el.tagName)) return null;
    const node = { id: nextId++, tag: el.tagName.toLowerCase(), box: box(el.getBoundingClientRect()), style: {}, kids: [] };
    for (const k of KEEP) node.style[k] = cs.getPropertyValue(k);
    if (el.tagName === 'IMG') node.src = el.currentSrc || el.src;
    if (el.tagName === 'TD' || el.tagName === 'TH') { node.colspan = el.colSpan; node.rowspan = el.rowSpan; }
    // ::before 글자(체크 칸 "☐ " 등)는 DOM 글자가 아니라 따로 받는다
    const bc = getComputedStyle(el, '::before').content;
    if (bc && bc !== 'none' && bc !== 'normal' && /^".*"$/.test(bc)) node.before = JSON.parse(bc);
    for (const c of el.childNodes) {
      if (c.nodeType === 1) {
        const k = walk(c, node);
        if (k) node.kids.push(k);
      } else if (c.nodeType === 3 && c.textContent.trim()) {
        // 글자 하나하나의 줄 위치로 Edge가 실제로 나눈 줄을 되살린다
        const range = document.createRange();
        const t = c.textContent;
        let line = null;
        const lines = [];
        for (let i = 0; i < t.length; i++) {
          range.setStart(c, i);
          range.setEnd(c, i + 1);
          const rs = range.getClientRects();
          if (!rs.length) { if (line) line.text += t[i]; continue; }
          const b = rs[0];
          if (line && Math.abs(b.top - line._top) < 2) {
            line.text += t[i];
            line.box.w = r(b.right + scrollX - line.box.x);
          } else {
            if (line) lines.push(line);
            line = { text: t[i], _top: b.top, box: { x: r(b.left + scrollX), y: r(b.top + scrollY), w: r(b.width), h: r(b.height) } };
          }
        }
        if (line) lines.push(line);
        for (const l of lines) delete l._top;
        node.kids.push({ tag: '#text', lines });
      }
    }
    return node;
  }
  const out = {
    page: { w: document.documentElement.scrollWidth, h: document.documentElement.scrollHeight },
    root: walk(document.body, null),
  };
  const pre = document.createElement('pre');
  pre.id = 'octodocs-dump';
  pre.textContent = JSON.stringify(out);
  document.documentElement.appendChild(pre);
})();
