//! md·txt 엔진 (3절 기능 4a): 변환 없이 텍스트 그대로 열고 고쳐 저장한다.
//! 원래 인코딩(BOM 포함)과 줄마다의 줄바꿈을 그대로 지켜, 손대지 않은 줄은 바이트까지 같다.
//! 문단 = 줄. `section` 은 언제나 0.

use encoding_rs::{Encoding, EUC_KR, UTF_16BE, UTF_16LE, UTF_8};
use serde_json::{json, Value};

use crate::p::{b_or, byte_at, char_matches, st, us, us_or};
use crate::{Engine, Format};

pub struct TextEngine {
    fmt: Format,
    enc: &'static Encoding,
    bom: Vec<u8>,
    /// (줄 글자, 그 줄의 줄바꿈 "\r\n" | "\n" | "\r" | "")
    lines: Vec<(String, String)>,
    /// 새 줄에 붙일 줄바꿈(파일에서 가장 먼저 나온 것)
    eol: String,
}

fn decode(data: &[u8]) -> (&'static Encoding, Vec<u8>, String) {
    if let Some((enc, bom_len)) = Encoding::for_bom(data) {
        let (text, _) = enc.decode_without_bom_handling(&data[bom_len..]);
        return (enc, data[..bom_len].to_vec(), text.into_owned());
    }
    match std::str::from_utf8(data) {
        Ok(s) => (UTF_8, Vec::new(), s.to_string()),
        // UTF-8이 아니면 한국어 문서에서 가장 흔한 CP949(EUC-KR 상위 집합)로 읽는다
        Err(_) => {
            let (text, _, _) = EUC_KR.decode(data);
            (EUC_KR, Vec::new(), text.into_owned())
        }
    }
}

fn split_lines(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\r' if it.peek() == Some(&'\n') => {
                it.next();
                out.push((std::mem::take(&mut cur), "\r\n".to_string()));
            }
            '\r' => out.push((std::mem::take(&mut cur), "\r".to_string())),
            '\n' => out.push((std::mem::take(&mut cur), "\n".to_string())),
            c => cur.push(c),
        }
    }
    if !cur.is_empty() || out.is_empty() {
        out.push((cur, String::new()));
    }
    out
}

impl TextEngine {
    pub fn open(fmt: Format, data: Vec<u8>) -> Result<Self, String> {
        let (enc, bom, text) = decode(&data);
        let lines = split_lines(&text);
        let eol = lines.iter().map(|l| l.1.clone()).find(|e| !e.is_empty()).unwrap_or_else(|| "\n".to_string());
        Ok(TextEngine { fmt, enc, bom, lines, eol })
    }

    fn line(&self, p: &Value) -> Result<usize, String> {
        let i = us(p, "para")?;
        if us_or(p, "section", 0) != 0 {
            return Err("md·txt에는 구역이 하나(0)뿐입니다".into());
        }
        if i >= self.lines.len() {
            return Err(format!("{i}번 줄이 없습니다(줄 {}개)", self.lines.len()));
        }
        Ok(i)
    }

    /// 새 줄을 i 자리에 넣는다. 마지막 줄 뒤에 붙일 때는 앞줄에 줄바꿈을 준다.
    fn insert_line(&mut self, i: usize, text: String) {
        if i >= self.lines.len() {
            if let Some(last) = self.lines.last_mut() {
                if last.1.is_empty() {
                    last.1 = self.eol.clone();
                }
            }
            self.lines.push((text, String::new()));
        } else {
            self.lines.insert(i, (text, self.eol.clone()));
        }
    }

    fn wrap(&mut self, p: &Value, mark: &str) -> Result<Value, String> {
        if self.fmt != Format::Markdown {
            return Err("txt에는 글자 모양이 없습니다(md만 ** · * 로 감쌉니다)".into());
        }
        if !b_or(p, "on", true) {
            return Err("md 서식 풀기는 지원하지 않습니다".into());
        }
        let i = self.line(p)?;
        let (s, e) = (us(p, "start")?, us(p, "end")?);
        let t = &mut self.lines[i].0;
        let (bs, be) = (byte_at(t, s), byte_at(t, e));
        t.insert_str(be, mark);
        t.insert_str(bs, mark);
        Ok(json!({"ok": true}))
    }
}

impl Engine for TextEngine {
    fn format(&self) -> Format {
        self.fmt
    }

    fn run(&mut self, id: &str, p: &Value) -> Result<Value, String> {
        Ok(match id {
            "doc.info" => json!({
                "format": self.fmt.name(),
                "encoding": self.enc.name(),
                "bom": !self.bom.is_empty(),
                "lines": self.lines.len(),
                "eol": match self.eol.as_str() { "\r\n" => "CRLF", "\r" => "CR", _ => "LF" },
            }),
            "doc.text" => Value::Array(
                self.lines
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| !l.0.is_empty())
                    .map(|(i, l)| json!({"section": 0, "para": i, "text": l.0}))
                    .collect(),
            ),
            "para.text" => json!({"text": self.lines[self.line(p)?].0}),
            "para.insert" => {
                let i = us(p, "para")?;
                self.insert_line(i.min(self.lines.len()), String::new());
                json!({"para": i})
            }
            "para.delete" => {
                let i = self.line(p)?;
                let removed = self.lines.remove(i);
                // 마지막 줄을 지웠으면 새 마지막 줄의 줄바꿈 유무를 원래 마지막 줄에 맞춘다
                if i == self.lines.len() {
                    if let Some(last) = self.lines.last_mut() {
                        last.1 = removed.1;
                    }
                }
                if self.lines.is_empty() {
                    self.lines.push((String::new(), String::new()));
                }
                json!({"ok": true})
            }
            "para.append" => {
                let i = self.line(p)?;
                let text = p.get("text").and_then(Value::as_str).unwrap_or("").to_string();
                self.insert_line(i + 1, text);
                json!({"para": i + 1})
            }
            "text.insert" => {
                let i = self.line(p)?;
                let (off, text) = (us(p, "offset")?, st(p, "text")?);
                let b = byte_at(&self.lines[i].0, off);
                self.lines[i].0.insert_str(b, text);
                json!({"ok": true})
            }
            "text.delete" => {
                let i = self.line(p)?;
                let (off, n) = (us(p, "offset")?, us(p, "count")?);
                let t = &mut self.lines[i].0;
                let (bs, be) = (byte_at(t, off), byte_at(t, off + n));
                t.replace_range(bs..be, "");
                json!({"ok": true})
            }
            "text.append" => {
                let i = self.line(p)?;
                self.lines[i].0.push_str(st(p, "text")?);
                json!({"ok": true})
            }
            "text.find" => {
                let find = st(p, "find")?;
                let mut out = Vec::new();
                for (i, l) in self.lines.iter().enumerate() {
                    for off in char_matches(&l.0, find) {
                        out.push(json!({"section": 0, "para": i, "offset": off}));
                    }
                }
                Value::Array(out)
            }
            "text.replace" => {
                let (find, rep) = (st(p, "find")?, st(p, "replace")?);
                if find.is_empty() {
                    return Err("find는 비어 있으면 안 됩니다".into());
                }
                let mut n = 0;
                for l in self.lines.iter_mut() {
                    let c = l.0.matches(find).count();
                    if c > 0 {
                        n += c;
                        l.0 = l.0.replace(find, rep);
                    }
                }
                json!({"replaced": n})
            }
            "format.bold" => self.wrap(p, "**")?,
            "format.italic" => self.wrap(p, "*")?,
            "table.list" => json!([]),
            id if id.starts_with("table.") => return Err("md·txt에는 표 명령을 쓸 수 없습니다".into()),
            "format.underline" | "para.align" => return Err(format!("{id}: md·txt에는 없는 서식입니다")),
            other => return Err(format!("모르는 명령입니다: {other}")),
        })
    }

    fn save(&mut self) -> Result<(Vec<u8>, Value), String> {
        let mut s = String::new();
        for (t, e) in &self.lines {
            s.push_str(t);
            s.push_str(e);
        }
        let mut out = self.bom.clone();
        if self.enc == UTF_16LE || self.enc == UTF_16BE {
            let le = self.enc == UTF_16LE;
            for u in s.encode_utf16() {
                out.extend_from_slice(&if le { u.to_le_bytes() } else { u.to_be_bytes() });
            }
        } else {
            let (bytes, _, unmappable) = self.enc.encode(&s);
            if unmappable {
                return Err(format!("{} 인코딩으로 쓸 수 없는 글자가 있습니다", self.enc.name()));
            }
            out.extend_from_slice(&bytes);
        }
        Ok((out, json!({"encoding": self.enc.name(), "bom": !self.bom.is_empty()})))
    }

    fn native_commands(&self) -> Vec<(String, String, String)> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eng(bytes: &[u8]) -> TextEngine {
        TextEngine::open(Format::Text, bytes.to_vec()).unwrap()
    }

    #[test]
    fn untouched_lines_and_mixed_eols_survive() {
        let src = b"one\r\ntwo\nthree\rfour";
        let mut e = eng(src);
        e.run("text.append", &json!({"para":1,"text":"!"})).unwrap();
        let (out, _) = e.save().unwrap();
        assert_eq!(out, b"one\r\ntwo!\nthree\rfour");
    }

    #[test]
    fn bom_and_trailing_newline_kept() {
        let src = b"\xEF\xBB\xBFa\r\nb\r\n";
        let mut e = eng(src);
        e.run("para.append", &json!({"para":1,"text":"c"})).unwrap();
        let (out, _) = e.save().unwrap();
        assert_eq!(out, b"\xEF\xBB\xBFa\r\nb\r\nc\r\n".to_vec());
    }

    #[test]
    fn cp949_round_trip() {
        let (bytes, _, _) = EUC_KR.encode("가나다\r\n라마");
        let mut e = eng(&bytes);
        assert_eq!(e.run("doc.info", &json!({})).unwrap()["encoding"], "EUC-KR");
        e.run("text.replace", &json!({"find":"라마","replace":"바사"})).unwrap();
        let (out, _) = e.save().unwrap();
        assert_eq!(EUC_KR.decode(&out).0, "가나다\r\n바사");
    }

    #[test]
    fn utf16_round_trip() {
        let mut src = vec![0xFF, 0xFE];
        for u in "가\r\n나".encode_utf16() {
            src.extend_from_slice(&u.to_le_bytes());
        }
        let mut e = eng(&src);
        let (out, _) = e.save().unwrap();
        assert_eq!(out, src);
        assert_eq!(e.run("para.text", &json!({"para":1})).unwrap()["text"], "나");
    }

    #[test]
    fn markdown_bold_wraps() {
        let mut e = TextEngine::open(Format::Markdown, "제목 글".as_bytes().to_vec()).unwrap();
        e.run("format.bold", &json!({"para":0,"start":0,"end":2})).unwrap();
        assert_eq!(e.run("para.text", &json!({"para":0})).unwrap()["text"], "**제목** 글");
    }

    #[test]
    fn text_has_no_tables() {
        assert!(eng(b"x").run("table.setCell", &json!({})).is_err());
    }
}
