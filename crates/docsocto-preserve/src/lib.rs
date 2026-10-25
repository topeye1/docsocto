//! 보존 저장 (기획서 9절 대책, 진행일지 A3).
//!
//! 엔진(rhwp·WordCraft)은 저장할 때 패키지 전체를 다시 쓴다. 그래서 손대지 않은 부분까지
//! 원본과 달라지고, 엔진이 모르는 항목(그림·머리말 연결 등)이 빠지기도 한다.
//! 여기서는 세 벌을 맞대어 바뀐 부분만 원본에 끼운다.
//!
//! - `O`  : 원본 패키지
//! - `B0` : 엔진이 아무것도 고치지 않고 저장한 것
//! - `B1` : 엔진이 고쳐서 저장한 것
//!
//! 항목(part)이 B0와 B1에서 같으면 편집과 무관하므로 원본 O를 그대로 쓴다. 다르면 XML의
//! 문단 단위 자식(HWPX `hs:sec` 아래 `hp:p`, docx `w:body` 아래 `w:p`·`w:tbl` 등)으로
//! B0와 B1을 맞대어, 바뀐 자식만 B1에서 가져와 O의 같은 자리에 끼운다. 맞댈 수 없으면 그
//! 항목만 B1 전체를 쓰고 보고서에 남긴다.

use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read, Write};
use std::ops::Range;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, Writer};
use serde::Serialize;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

#[derive(Debug)]
pub struct Entry {
    pub name: String,
    pub data: Vec<u8>,
    pub method: CompressionMethod,
}

#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

fn err<E: std::fmt::Display>(what: &str) -> impl FnOnce(E) -> Error + '_ {
    move |e| Error(format!("{what}: {e}"))
}

pub fn read_package(bytes: &[u8]) -> Result<Vec<Entry>, Error> {
    let mut z = ZipArchive::new(Cursor::new(bytes)).map_err(err("zip 열기"))?;
    let mut out = Vec::with_capacity(z.len());
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(err("zip 항목"))?;
        if f.is_dir() {
            continue;
        }
        let mut data = Vec::with_capacity(f.size() as usize);
        f.read_to_end(&mut data).map_err(err("zip 읽기"))?;
        out.push(Entry { name: f.name().to_string(), data, method: f.compression() });
    }
    Ok(out)
}

pub fn write_package(entries: &[Entry]) -> Result<Vec<u8>, Error> {
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    for e in entries {
        let method = match e.method {
            CompressionMethod::Stored => CompressionMethod::Stored,
            _ => CompressionMethod::Deflated,
        };
        w.start_file(e.name.as_str(), SimpleFileOptions::default().compression_method(method))
            .map_err(err("zip 쓰기"))?;
        w.write_all(&e.data).map_err(err("zip 쓰기"))?;
    }
    Ok(w.finish().map_err(err("zip 마무리"))?.into_inner())
}

/// 한 XML 항목을 "머리 + (틈, 자식)* + 끝 틈 + 꼬리"로 나눈 것.
#[derive(Debug)]
struct Split {
    head: Range<usize>,
    /// (자식 앞 틈, 자식) — 틈은 공백·주석처럼 요소가 아닌 것.
    kids: Vec<(Range<usize>, Range<usize>)>,
    last_gap: Range<usize>,
    tail: Range<usize>,
}

/// 문단 단위 자식이 들어 있는 요소를 찾는다: 기본은 루트, docx 본문은 `document` 아래 `body`.
fn split(xml: &[u8]) -> Option<Split> {
    let mut r = Reader::from_reader(xml);
    r.config_mut().trim_text(false);
    let mut depth = 0usize;
    let mut container: Option<usize> = None; // 자식들이 놓인 깊이
    let mut content_start = 0usize;
    let mut kids = Vec::new();
    let mut kid_start = 0usize;
    let mut gap_start = 0usize;
    loop {
        let pos = r.buffer_position() as usize;
        let ev = r.read_event().ok()?;
        let end = r.buffer_position() as usize;
        match ev {
            Event::Start(e) => {
                depth += 1;
                match container {
                    None => {
                        let local = e.local_name();
                        let is_doc_root = depth == 1 && local.as_ref() == "document";
                        if depth == 1 && !is_doc_root || depth == 2 && local.as_ref() == "body" {
                            container = Some(depth + 1);
                            content_start = end;
                            gap_start = end;
                        }
                    }
                    Some(c) if depth == c => kid_start = pos,
                    _ => {}
                }
            }
            Event::Empty(_) => {
                if container == Some(depth + 1) {
                    kids.push((gap_start..pos, pos..end));
                    gap_start = end;
                }
            }
            Event::End(_) => {
                match container {
                    Some(c) if depth == c => {
                        kids.push((gap_start..kid_start, kid_start..end));
                        gap_start = end;
                    }
                    Some(c) if depth + 1 == c => {
                        return Some(Split {
                            head: 0..content_start,
                            kids,
                            last_gap: gap_start..pos,
                            tail: pos..xml.len(),
                        });
                    }
                    _ => {}
                }
                depth = depth.checked_sub(1)?;
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    /// 양쪽에 있는 짝 (a 쪽 번호, b 쪽 번호)
    Both(usize, usize),
    AOnly(usize),
    BOnly(usize),
}

/// 두 목록을 맞댄다. 앞뒤 공통 부분을 먼저 잘라 내고 가운데만 LCS로 맞춘다.
/// 가운데가 너무 크면 None (호출한 쪽이 다른 방법을 쓴다).
fn align<T: PartialEq>(a: &[T], b: &[T]) -> Option<Vec<Step>> {
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len() - pre && suf < b.len() - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf] {
        suf += 1;
    }
    let (ma, mb) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    let (n, m) = (ma.len(), mb.len());
    let mut steps: Vec<Step> = (0..pre).map(|i| Step::Both(i, i)).collect();
    if n.saturating_mul(m) > 16_000_000 {
        return None;
    }
    let mut t = vec![0u32; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i * (m + 1) + j] = if ma[i] == mb[j] {
                t[(i + 1) * (m + 1) + j + 1] + 1
            } else {
                t[(i + 1) * (m + 1) + j].max(t[i * (m + 1) + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && ma[i] == mb[j] {
            steps.push(Step::Both(pre + i, pre + j));
            i += 1;
            j += 1;
        } else if j < m && (i == n || t[i * (m + 1) + j + 1] >= t[(i + 1) * (m + 1) + j]) {
            steps.push(Step::BOnly(pre + j));
            j += 1;
        } else {
            steps.push(Step::AOnly(pre + i));
            i += 1;
        }
    }
    steps.extend((0..suf).map(|k| Step::Both(a.len() - suf + k, b.len() - suf + k)));
    Some(steps)
}

/// 자식 하나의 "내용 표지": 요소 이름 + 안의 글자 전부. 엔진이 속성·모양을 다르게 써도
/// 같은 문단이면 같은 표지가 나오므로 원본 자식과 엔진 자식을 짝짓는 데 쓴다.
fn signature(snippet: &[u8]) -> Vec<u8> {
    let mut r = Reader::from_reader(snippet);
    r.config_mut().trim_text(false);
    let mut out = Vec::new();
    let mut first = true;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if first => {
                out.extend_from_slice(e.local_name().as_ref().as_bytes());
                out.push(0);
                first = false;
            }
            Ok(Event::Text(t)) => out.extend_from_slice(t.as_ref().as_bytes()),
            Ok(Event::CData(t)) => out.extend_from_slice(t.as_ref().as_bytes()),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

#[derive(Debug, Default, Serialize)]
pub struct PartReport {
    pub name: String,
    /// original | merged | engine | dropped | added | skipped
    pub action: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taken: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub parts: Vec<PartReport>,
    pub rels_added: usize,
    /// 맞대지 못한 항목이 있어 엔진 결과 전체를 쓴 경우
    pub whole_engine: bool,
}

/// 편집과 상관없는 문서 정보 항목(작성일·쪽 수·미리보기) — 언제나 원본을 쓴다.
fn is_metadata(name: &str) -> bool {
    name.starts_with("docProps/") || name.starts_with("Preview/")
}

/// docx 관계(rels) 한 벌: Id → (Type, Target, TargetMode).
type Rels = BTreeMap<String, (String, String, String)>;

fn parse_rels(xml: &[u8]) -> Rels {
    let mut out = Rels::new();
    let mut r = Reader::from_reader(xml);
    while let Ok(ev) = r.read_event() {
        match ev {
            Event::Empty(e) | Event::Start(e) if e.local_name().as_ref() == "Relationship" => {
                let get = |k: &str| {
                    e.attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == k)
                        .map(|a| a.value.to_string())
                        .unwrap_or_default()
                };
                out.insert(get("Id"), (get("Type"), get("Target"), get("TargetMode")));
            }
            Event::Eof => break,
            _ => {}
        }
    }
    out
}

/// `word/document.xml` → `word/_rels/document.xml.rels`
fn rels_name(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

const REL_ATTRS: &[&str] = &["id", "embed", "link", "pict", "href", "dm", "lo", "qs", "cs"];

/// 끼울 자식 안의 관계 id(r:id·r:embed …)를 B1 번호에서 원본 O 번호로 바꾼다.
/// O에 같은 대상이 없으면 새 관계를 `added`에 쌓는다.
struct RelMap<'a> {
    b1: &'a Rels,
    o_by_target: HashMap<(String, String), String>,
    o_ids: std::collections::HashSet<String>,
    added: Vec<(String, (String, String, String))>,
}

impl<'a> RelMap<'a> {
    fn new(o: &Rels, b1: &'a Rels) -> Self {
        RelMap {
            b1,
            o_by_target: o.iter().map(|(id, (t, tg, _))| ((t.clone(), tg.clone()), id.clone())).collect(),
            o_ids: o.keys().cloned().collect(),
            added: Vec::new(),
        }
    }

    fn map(&mut self, id: &str) -> Option<String> {
        let rel = self.b1.get(id)?;
        let key = (rel.0.clone(), rel.1.clone());
        if let Some(o) = self.o_by_target.get(&key) {
            return Some(o.clone());
        }
        let mut n = self.o_ids.len() + 1;
        let new_id = loop {
            let c = format!("rIdDo{n}");
            if !self.o_ids.contains(&c) {
                break c;
            }
            n += 1;
        };
        self.o_ids.insert(new_id.clone());
        self.o_by_target.insert(key, new_id.clone());
        self.added.push((new_id.clone(), rel.clone()));
        Some(new_id)
    }

    fn rewrite(&mut self, snippet: &[u8]) -> Vec<u8> {
        let mut r = Reader::from_reader(snippet);
        r.config_mut().trim_text(false);
        let mut w = Writer::new(Vec::with_capacity(snippet.len()));
        let mut changed = false;
        loop {
            let ev = match r.read_event() {
                Ok(Event::Eof) => break,
                Ok(ev) => ev,
                Err(_) => return snippet.to_vec(),
            };
            let ev = match ev {
                Event::Start(e) => Event::Start(self.fix(e, &mut changed)),
                Event::Empty(e) => Event::Empty(self.fix(e, &mut changed)),
                other => other,
            };
            if w.write_event(ev).is_err() {
                return snippet.to_vec();
            }
        }
        if changed { w.into_inner() } else { snippet.to_vec() }
    }

    fn fix<'b>(&mut self, e: BytesStart<'b>, changed: &mut bool) -> BytesStart<'b> {
        let needs = e.attributes().flatten().any(|a| {
            a.key.prefix().map(|p| p.as_ref() == "r").unwrap_or(false) && REL_ATTRS.contains(&a.key.local_name().as_ref())
        });
        if !needs {
            return e;
        }
        let name = e.name().as_ref().to_string();
        let mut out = BytesStart::new(name);
        for a in e.attributes().flatten() {
            let key = a.key.as_ref().to_string();
            let val = a.value.to_string();
            let is_rel = a.key.prefix().map(|p| p.as_ref() == "r").unwrap_or(false)
                && REL_ATTRS.contains(&a.key.local_name().as_ref());
            let val = match is_rel.then(|| self.map(&val)).flatten() {
                Some(v) if v != val => {
                    *changed = true;
                    v
                }
                _ => val,
            };
            out.push_attribute((key.as_str(), val.as_str()));
        }
        out.into_owned()
    }
}

fn is_xml(name: &str) -> bool {
    name.ends_with(".xml") || name.ends_with(".rels") || name.ends_with(".hpf")
}

/// 자식 단위 병합. 맞댈 수 없으면 Err(이유).
///
/// 1) 원본 O 자식과 B0 자식을 내용 표지(요소 이름 + 글자)로 짝짓는다. O에만 있는 자식은
///    엔진이 모르는 것(책갈피 등)이므로 제자리에 그대로 남긴다.
/// 2) B0 자식과 B1 자식을 바이트로 맞대어 바뀐 것을 찾는다.
/// 3) 짝이 있고 안 바뀐 자식은 O의 바이트를, 바뀌거나 새로 생긴 자식은 B1의 바이트를 쓴다.
fn merge_xml(o: &[u8], b0: &[u8], b1: &[u8], rel: Option<&mut RelMap>) -> Result<(Vec<u8>, usize, usize, usize), String> {
    let (so, s0, s1) = match (split(o), split(b0), split(b1)) {
        (Some(a), Some(b), Some(c)) => (a, b, c),
        _ => return Err("XML 구조를 나누지 못함".into()),
    };
    if b0[s0.head.clone()] != b1[s1.head.clone()] || b0[s0.tail.clone()] != b1[s1.tail.clone()] {
        return Err("머리(루트 시작 태그 등)나 꼬리가 바뀜".into());
    }
    let ko: Vec<&[u8]> = so.kids.iter().map(|(_, k)| &o[k.clone()]).collect();
    let k0: Vec<&[u8]> = s0.kids.iter().map(|(_, k)| &b0[k.clone()]).collect();
    let k1: Vec<&[u8]> = s1.kids.iter().map(|(_, k)| &b1[k.clone()]).collect();

    // 1) O ↔ B0
    let sig_o: Vec<Vec<u8>> = ko.iter().map(|k| signature(k)).collect();
    let sig_0: Vec<Vec<u8>> = k0.iter().map(|k| signature(k)).collect();
    let ob = align(&sig_o, &sig_0).ok_or("원본과 엔진 저장본의 문단이 너무 많아 맞대지 못함")?;
    let matched = ob.iter().filter(|s| matches!(s, Step::Both(..))).count();
    // 짝이 거의 없으면 맞댄 것이 아니다.
    if k0.len() > 4 && matched * 2 < k0.len() {
        return Err(format!("원본 자식 {}개 중 엔진 저장본과 짝지은 것이 {matched}개뿐", ko.len()));
    }

    // 2) B0 ↔ B1: B0 자식마다 그대로인지, 그 앞에 끼울 B1 자식은 무엇인지
    let bb = align(&k0, &k1).unwrap_or_else(|| {
        (0..k0.len()).map(Step::AOnly).chain((0..k1.len()).map(Step::BOnly)).collect()
    });
    let mut unchanged = vec![false; k0.len()];
    let mut before: Vec<Vec<usize>> = vec![Vec::new(); k0.len() + 1];
    let mut pending: Vec<usize> = Vec::new();
    for s in &bb {
        match *s {
            Step::Both(i, _) => {
                unchanged[i] = true;
                before[i].append(&mut pending);
            }
            Step::AOnly(i) => before[i].append(&mut pending),
            Step::BOnly(j) => pending.push(j),
        }
    }
    before[k0.len()].append(&mut pending);

    // 3) 원본 순서대로 내보낸다
    let mut out = Vec::with_capacity(o.len() + b1.len() / 4);
    out.extend_from_slice(&o[so.head.clone()]);
    let (mut kept, mut taken) = (0, 0);
    let mut rel = rel;
    let mut emit_b1 = |out: &mut Vec<u8>, j: usize, taken: &mut usize| {
        let (g, k) = &s1.kids[j];
        out.extend_from_slice(&b1[g.clone()]);
        match rel.as_deref_mut() {
            Some(m) => out.extend_from_slice(&m.rewrite(&b1[k.clone()])),
            None => out.extend_from_slice(&b1[k.clone()]),
        }
        *taken += 1;
    };
    for s in &ob {
        match *s {
            Step::AOnly(i) => {
                // 엔진이 모르는 원본 자식 — 그대로
                let (g, k) = &so.kids[i];
                out.extend_from_slice(&o[g.clone()]);
                out.extend_from_slice(&o[k.clone()]);
                kept += 1;
            }
            Step::Both(i, j0) => {
                for &j in &before[j0] {
                    emit_b1(&mut out, j, &mut taken);
                }
                if unchanged[j0] {
                    let (g, k) = &so.kids[i];
                    out.extend_from_slice(&o[g.clone()]);
                    out.extend_from_slice(&o[k.clone()]);
                    kept += 1;
                }
            }
            Step::BOnly(j0) => {
                // 엔진이 지어낸 자식 — 그대로면 버리고, 앞에 끼울 것만 내보낸다
                for &j in &before[j0] {
                    emit_b1(&mut out, j, &mut taken);
                }
            }
        }
    }
    for &j in &before[k0.len()] {
        emit_b1(&mut out, j, &mut taken);
    }
    out.extend_from_slice(&o[so.last_gap.clone()]);
    out.extend_from_slice(&o[so.tail.clone()]);
    let removed = so.kids.len() - kept;
    Ok((out, kept, taken, removed))
}

/// 세 벌을 받아 보존 저장한 패키지와 보고서를 돌려준다.
pub fn preserve(o: &[u8], b0: &[u8], b1: &[u8]) -> Result<(Vec<u8>, Report), Error> {
    let po = read_package(o)?;
    let p0 = read_package(b0)?;
    let p1 = read_package(b1)?;
    let m0: HashMap<&str, &Entry> = p0.iter().map(|e| (e.name.as_str(), e)).collect();
    let m1: HashMap<&str, &Entry> = p1.iter().map(|e| (e.name.as_str(), e)).collect();
    let mo: HashMap<&str, &Entry> = po.iter().map(|e| (e.name.as_str(), e)).collect();
    let is_docx = mo.contains_key("[Content_Types].xml");

    let mut report = Report::default();
    let mut out: Vec<Entry> = Vec::new();
    let mut whole_engine = false;
    // rels 항목 이름 → 덧붙일 관계
    let mut rel_adds: BTreeMap<String, Vec<(String, (String, String, String))>> = BTreeMap::new();

    for e in &po {
        let name = e.name.as_str();
        let mut pr = PartReport { name: name.to_string(), ..Default::default() };
        let (x0, x1) = (m0.get(name), m1.get(name));
        let data = match (x0, x1) {
            (Some(a), Some(b)) if a.data == b.data => {
                pr.action = "original";
                Some(e.data.clone())
            }
            (None, None) => {
                pr.action = "original";
                pr.note = Some("엔진이 빠뜨리는 항목 — 원본 유지".into());
                Some(e.data.clone())
            }
            (Some(_), None) => {
                pr.action = "dropped";
                pr.note = Some("편집으로 없어진 항목".into());
                None
            }
            (None, Some(b)) => {
                pr.action = "engine";
                pr.note = Some("고치지 않은 저장본에는 없던 항목".into());
                Some(b.data.clone())
            }
            (Some(_), Some(_)) if is_metadata(name) => {
                pr.action = "original";
                pr.note = Some("문서 정보(작성일·쪽 수 등) — 원본 유지".into());
                Some(e.data.clone())
            }
            (Some(a), Some(b)) => {
                let rels_o = is_docx.then(|| parse_rels(mo.get(rels_name(name).as_str()).map(|x| x.data.as_slice()).unwrap_or(b"")));
                let rels_1 = is_docx.then(|| parse_rels(m1.get(rels_name(name).as_str()).map(|x| x.data.as_slice()).unwrap_or(b"")));
                let mut rm = match (&rels_o, &rels_1) {
                    (Some(ro), Some(r1)) if !name.ends_with(".rels") => Some(RelMap::new(ro, r1)),
                    _ => None,
                };
                let merged = if is_xml(name) { merge_xml(&e.data, &a.data, &b.data, rm.as_mut()) } else { Err("XML 아님".into()) };
                match merged {
                    Ok((d, kept, taken, removed)) => {
                        pr.action = "merged";
                        pr.kept = Some(kept);
                        pr.taken = Some(taken);
                        pr.removed = Some(removed);
                        if let Some(m) = rm {
                            if !m.added.is_empty() {
                                rel_adds.entry(rels_name(name)).or_default().extend(m.added);
                            }
                        }
                        Some(d)
                    }
                    Err(why) => {
                        // 엔진이 쓴 항목 하나를 원본 항목들 사이에 섞으면 서로 가리키는 id가 어긋나
                        // 파일이 깨진다(M1-1 검증에서 Word "파일이 손상" 확인). 이때는 엔진 결과 전체를 쓴다.
                        pr.action = "engine";
                        pr.note = Some(why);
                        whole_engine = true;
                        Some(b.data.clone())
                    }
                }
            }
        };
        if let Some(data) = data {
            out.push(Entry { name: e.name.clone(), data, method: e.method });
        }
        report.parts.push(pr);
    }
    if whole_engine {
        report.whole_engine = true;
        return Ok((b1.to_vec(), report));
    }
    // 편집으로 새로 생긴 항목(고치지 않은 저장본에 없던 것)만 더한다.
    for e in &p1 {
        if !mo.contains_key(e.name.as_str()) && !m0.contains_key(e.name.as_str()) {
            out.push(Entry { name: e.name.clone(), data: e.data.clone(), method: e.method });
            report.parts.push(PartReport { name: e.name.clone(), action: "added", ..Default::default() });
        }
    }
    // 끼운 문단이 원본에 없는 관계를 가리키면 rels에 덧붙인다.
    for (rels, adds) in rel_adds {
        let Some(ent) = out.iter_mut().find(|e| e.name == rels) else { continue };
        let Some(pos) = find_last(&ent.data, b"</Relationships>") else { continue };
        let mut extra = Vec::new();
        for (id, (t, tg, mode)) in &adds {
            let mode = if mode.is_empty() { String::new() } else { format!(" TargetMode=\"{}\"", esc(mode)) };
            extra.extend_from_slice(
                format!("<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"{mode}/>", esc(id), esc(t), esc(tg)).as_bytes(),
            );
        }
        ent.data.splice(pos..pos, extra);
        report.rels_added += adds.len();
    }
    Ok((write_package(&out)?, report))
}

fn find_last(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).rposition(|w| w == needle)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kids(x: &str) -> Vec<String> {
        let s = split(x.as_bytes()).unwrap();
        s.kids.iter().map(|(_, k)| x[k.clone()].to_string()).collect()
    }

    #[test]
    fn split_root_children() {
        let x = r#"<?xml version="1.0"?><hs:sec xmlns:hs="s"><hp:p a="1">x</hp:p>
<hp:p a="2"/><!--c--><hp:p a="3"><t>y</t></hp:p></hs:sec>"#;
        assert_eq!(kids(x), vec![r#"<hp:p a="1">x</hp:p>"#, r#"<hp:p a="2"/>"#, r#"<hp:p a="3"><t>y</t></hp:p>"#]);
    }

    #[test]
    fn split_docx_body() {
        let x = r#"<w:document xmlns:w="w"><w:body><w:p>a</w:p><w:tbl><w:tr/></w:tbl><w:sectPr/></w:body></w:document>"#;
        assert_eq!(kids(x), vec!["<w:p>a</w:p>", "<w:tbl><w:tr/></w:tbl>", "<w:sectPr/>"]);
    }

    #[test]
    fn merge_keeps_original_bytes_for_untouched_children() {
        let o = r#"<r><p  id='1'>a</p>
  <p id='2'>b</p>
  <p id='3'>c</p></r>"#;
        let b0 = r#"<r><p id="1">a</p><p id="2">b</p><p id="3">c</p></r>"#;
        let b1 = r#"<r><p id="1">a</p><p id="2">B!</p><p id="3">c</p></r>"#;
        let (m, kept, taken, removed) = merge_xml(o.as_bytes(), b0.as_bytes(), b1.as_bytes(), None).unwrap();
        assert_eq!((kept, taken, removed), (2, 1, 1));
        assert_eq!(
            String::from_utf8(m).unwrap(),
            "<r><p  id='1'>a</p><p id=\"2\">B!</p>\n  <p id='3'>c</p></r>"
        );
    }

    #[test]
    fn merge_insertion() {
        let o = "<r><p>a</p><p>b</p></r>";
        let b1 = "<r><p>a</p><p>new</p><p>b</p></r>";
        let (m, ..) = merge_xml(o.as_bytes(), o.as_bytes(), b1.as_bytes(), None).unwrap();
        assert_eq!(String::from_utf8(m).unwrap(), b1);
    }

    #[test]
    fn child_unknown_to_engine_is_kept_in_place() {
        // 엔진이 책갈피를 빠뜨리고 문단 모양도 다르게 쓰지만, 원본 책갈피와 안 바뀐 문단은 그대로 남는다
        let o = r#"<r><p a='1'><t>a</t></p><bm n='x'/><p a='1'><t>b</t></p><p a='1'><t>c</t></p></r>"#;
        let b0 = r#"<r><p a="1"><t>a</t></p><p a="1"><t>b</t></p><p a="1"><t>c</t></p></r>"#;
        let b1 = r#"<r><p a="1"><t>a</t></p><p a="1"><t>B</t></p><p a="1"><t>c</t></p></r>"#;
        let (m, ..) = merge_xml(o.as_bytes(), b0.as_bytes(), b1.as_bytes(), None).unwrap();
        assert_eq!(
            String::from_utf8(m).unwrap(),
            r#"<r><p a='1'><t>a</t></p><bm n='x'/><p a="1"><t>B</t></p><p a='1'><t>c</t></p></r>"#
        );
    }

    #[test]
    fn rel_ids_are_mapped_to_original() {
        let o = parse_rels(br#"<Relationships><Relationship Id="rId5" Type="img" Target="media/a.png"/></Relationships>"#);
        let b1 = parse_rels(
            br#"<Relationships><Relationship Id="rId1" Type="img" Target="media/a.png"/><Relationship Id="rId2" Type="img" Target="media/b.png"/></Relationships>"#,
        );
        let mut m = RelMap::new(&o, &b1);
        let out = m.rewrite(br#"<w:p><a:blip r:embed="rId1"/><a:blip r:embed="rId2"/></w:p>"#);
        assert_eq!(String::from_utf8(out).unwrap(), r#"<w:p><a:blip r:embed="rId5"/><a:blip r:embed="rIdDo2"/></w:p>"#);
        assert_eq!(m.added.len(), 1);
    }
}
