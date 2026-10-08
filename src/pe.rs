// PE helpers and byte-pattern search for 32-bit game executables.
use std::collections::HashMap;

pub fn u32at(d: &[u8], o: usize) -> u32 { u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]) }
pub fn u16at(d: &[u8], o: usize) -> u16 { u16::from_le_bytes([d[o], d[o + 1]]) }
pub fn i32at(d: &[u8], o: usize) -> i32 { u32at(d, o) as i32 }
pub fn put32(d: &mut [u8], o: usize, v: u32) { d[o..o + 4].copy_from_slice(&v.to_le_bytes()); }
pub fn put16(d: &mut [u8], o: usize, v: u16) { d[o..o + 2].copy_from_slice(&v.to_le_bytes()); }
pub fn le(v: u32) -> [u8; 4] { v.to_le_bytes() }
pub fn rel32(from_next: u32, to: u32) -> [u8; 4] { (to.wrapping_sub(from_next)).to_le_bytes() }

#[derive(Clone, Debug)]
pub struct Sec { pub name: String, pub va: u32, pub vs: u32, pub rs: u32, pub ro: u32 }

#[derive(Clone, Debug)]
pub struct Pe { pub pe: usize, pub base: u32, pub secs: Vec<Sec> }

pub fn parse(d: &[u8]) -> Result<Pe, String> {
    if d.len() < 0x400 || &d[0..2] != b"MZ" { return Err("не исполняемый файл".into()); }
    let pe = u32at(d, 0x3c) as usize;
    if pe + 0x100 > d.len() || &d[pe..pe + 4] != b"PE\0\0" { return Err("не исполняемый файл".into()); }
    if u16at(d, pe + 24) != 0x10b { return Err("не 32-битный файл".into()); }
    let ns = u16at(d, pe + 6) as usize;
    let o0 = u16at(d, pe + 20) as usize;
    let base = u32at(d, pe + 52);
    let mut secs = vec![];
    for i in 0..ns {
        let o = pe + 24 + o0 + i * 40;
        let name = String::from_utf8_lossy(&d[o..o + 8]).trim_end_matches('\0').to_string();
        secs.push(Sec { name, vs: u32at(d, o + 8), va: u32at(d, o + 12), rs: u32at(d, o + 16), ro: u32at(d, o + 20) });
    }
    Ok(Pe { pe, base, secs })
}

impl Pe {
    pub fn sec(&self, n: &str) -> Option<&Sec> { self.secs.iter().find(|s| s.name == n) }
    pub fn text(&self) -> &Sec { self.sec(".text").unwrap_or(&self.secs[0]) }
    pub fn rva2off(&self, r: u32) -> Option<usize> {
        self.secs.iter().find(|s| s.va <= r && r < s.va + s.vs.max(s.rs)).map(|s| (r - s.va + s.ro) as usize)
    }
    pub fn o2v(&self, o: usize) -> u32 {
        let o = o as u32;
        for s in &self.secs { if s.ro <= o && o < s.ro + s.rs { return self.base + s.va + (o - s.ro); } }
        0
    }
    /// Addresses of IAT slots for imported functions (by name).
    pub fn iat(&self, d: &[u8], names: &[&str]) -> HashMap<String, u32> {
        let mut out = HashMap::new();
        let imp = u32at(d, self.pe + 24 + 104);
        let mut o = match self.rva2off(imp) { Some(o) => o, None => return out };
        loop {
            if o + 20 > d.len() { break; }
            let (oft, nm, ft) = (u32at(d, o), u32at(d, o + 12), u32at(d, o + 16));
            if nm == 0 { break; }
            let src = if oft != 0 { oft } else { ft };
            if let Some(so) = self.rva2off(src) {
                let mut k = 0usize;
                loop {
                    let e = u32at(d, so + k * 4);
                    if e == 0 { break; }
                    if e & 0x8000_0000 == 0 {
                        if let Some(no) = self.rva2off(e) {
                            let s = &d[no + 2..];
                            let end = s.iter().position(|&c| c == 0).unwrap_or(0);
                            let f = String::from_utf8_lossy(&s[..end]).to_string();
                            if names.contains(&f.as_str()) { out.insert(f, self.base + ft + (k as u32) * 4); }
                        }
                    }
                    k += 1;
                }
            }
            o += 20;
        }
        out
    }
}

/// Append a new RWX section; returns (absolute VA, raw offset).
pub fn add_section(d: &mut Vec<u8>, name: &str, size: u32) -> Result<(u32, usize), String> {
    let p = parse(d)?;
    if let Some(s) = p.sec(name) { return Ok((p.base + s.va, s.ro as usize)); }
    let pe = p.pe;
    let nsec = u16at(d, pe + 6) as usize;
    let opt = u16at(d, pe + 20) as usize;
    let sh = pe + 24 + opt;
    let salign = u32at(d, pe + 24 + 32);
    let falign = u32at(d, pe + 24 + 36);
    let first_raw = p.secs.iter().filter(|s| s.ro != 0).map(|s| s.ro).min().unwrap_or(0x400) as usize;
    if sh + (nsec + 1) * 40 > first_raw { return Err("нет места в заголовке".into()); }
    let last = p.secs.iter().max_by_key(|s| s.va).unwrap();
    let va = (last.va + last.vs.max(last.rs) + salign - 1) / salign * salign;
    let raw = (d.len() as u32 + falign - 1) / falign * falign;
    d.resize(raw as usize, 0);
    d.extend(std::iter::repeat(0xccu8).take(size as usize));
    let mut h = [0u8; 40];
    h[..name.len()].copy_from_slice(name.as_bytes());
    put32(&mut h, 8, size); put32(&mut h, 12, va); put32(&mut h, 16, size); put32(&mut h, 20, raw);
    put32(&mut h, 36, 0xE000_0020);
    d[sh + nsec * 40..sh + nsec * 40 + 40].copy_from_slice(&h);
    put16(d, pe + 6, (nsec + 1) as u16);
    put32(d, pe + 24 + 56, va + size);
    Ok((p.base + va, raw as usize))
}

// ---- pattern search: "8b 44 ?? [31 33] cc{50}" ----
#[derive(Clone)]
enum Tok { B(u8), Any, Set(Vec<u8>) }

pub struct Pat { t: Vec<Tok> }

impl Pat {
    pub fn new(s: &str) -> Pat {
        let mut t = vec![];
        let mut it = s.split_whitespace();
        while let Some(w) = it.next() {
            if w == "??" || w == "?" { t.push(Tok::Any); continue; }
            if let Some(r) = w.strip_prefix('[') {
                let mut v = vec![u8::from_str_radix(r.trim_end_matches(']'), 16).unwrap()];
                if !w.ends_with(']') {
                    for x in it.by_ref() {
                        v.push(u8::from_str_radix(x.trim_end_matches(']'), 16).unwrap());
                        if x.ends_with(']') { break; }
                    }
                }
                t.push(Tok::Set(v)); continue;
            }
            t.push(Tok::B(u8::from_str_radix(w, 16).unwrap()));
        }
        Pat { t }
    }
    pub fn len(&self) -> usize { self.t.len() }
    fn at(&self, d: &[u8], i: usize) -> bool {
        if i + self.t.len() > d.len() { return false; }
        self.t.iter().enumerate().all(|(k, t)| match t { Tok::B(b) => d[i + k] == *b, Tok::Any => true, Tok::Set(v) => v.contains(&d[i + k]) })
    }
    fn scan(&self, d: &[u8], from: usize, to: usize, only_first: bool) -> Vec<usize> {
        let mut r = vec![];
        let to = to.min(d.len());
        let n = self.t.len();
        let first = match self.t.first() { Some(Tok::B(b)) => Some(*b), _ => None };
        let mut i = from;
        while i + n <= to {
            if let Some(b) = first {
                match d[i..to].iter().position(|&c| c == b) { Some(p) => i += p, None => break }
                if i + n > to { break; }
            }
            if self.at(d, i) { r.push(i); if only_first { break; } i += n.max(1); } else { i += 1; }
        }
        r
    }
    pub fn find_in(&self, d: &[u8], from: usize, to: usize) -> Vec<usize> { self.scan(d, from, to, false) }
    pub fn all(&self, d: &[u8]) -> Vec<usize> { self.scan(d, 0, d.len(), false) }
    pub fn first(&self, d: &[u8], from: usize) -> Option<usize> { self.scan(d, from, d.len(), true).first().copied() }
}

pub fn one(d: &[u8], pat: &str, what: &str) -> Result<usize, String> {
    let v = Pat::new(pat).all(d);
    if v.len() != 1 { return Err(format!("{what}: найдено {}", v.len())); }
    Ok(v[0])
}
pub fn first(d: &[u8], pat: &str, from: usize, what: &str) -> Result<usize, String> {
    Pat::new(pat).first(d, from).ok_or_else(|| format!("{what}: не найдено"))
}

/// Start of the first run of at least `n` 0xCC bytes inside [from, to).
pub fn cc_run(d: &[u8], from: usize, to: usize, n: usize) -> Option<usize> {
    let mut run = 0usize;
    for i in from..to.min(d.len()) {
        if d[i] == 0xcc { run += 1; if run == n { return Some(i + 1 - n); } } else { run = 0; }
    }
    None
}

// ---- tiny assembler helper (labels + short/near jumps) ----
pub struct Asm { pub va: u32, pub b: Vec<u8>, lab: HashMap<&'static str, usize>, fix: Vec<(usize, &'static str, bool)> }
impl Asm {
    pub fn new(va: u32) -> Asm { Asm { va, b: vec![], lab: HashMap::new(), fix: vec![] } }
    pub fn e(&mut self, parts: &[&[u8]]) { for p in parts { self.b.extend_from_slice(p); } }
    pub fn l(&mut self, n: &'static str) { self.lab.insert(n, self.b.len()); }
    pub fn j(&mut self, op: &[u8], n: &'static str) { self.b.extend_from_slice(op); self.b.push(0); self.fix.push((self.b.len() - 1, n, false)); }
    pub fn jn(&mut self, op: &[u8], n: &'static str) { self.b.extend_from_slice(op); self.b.extend_from_slice(&[0; 4]); self.fix.push((self.b.len() - 4, n, true)); }
    pub fn rel(&mut self, t: u32) { let nx = self.va + self.b.len() as u32 + 4; self.b.extend_from_slice(&rel32(nx, t)); }
    pub fn done(mut self) -> Result<Vec<u8>, String> {
        for (pos, n, near) in self.fix.clone() {
            let t = *self.lab.get(n).ok_or("asm label")? as i64;
            if near { let v = (t - (pos as i64 + 4)) as i32; self.b[pos..pos + 4].copy_from_slice(&v.to_le_bytes()); }
            else {
                let v = t - (pos as i64 + 1);
                if !(-128..128).contains(&v) { return Err(format!("asm short jump {n}")); }
                self.b[pos] = v as i8 as u8;
            }
        }
        Ok(self.b)
    }
}
