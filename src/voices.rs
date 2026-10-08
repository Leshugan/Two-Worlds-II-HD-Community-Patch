// Russian voice-over fix: English lines that also exist in Russian are hidden (name prefix changed).
use std::collections::HashSet;
use std::fs;
use std::path::Path;

const RUS: [&str; 3] = ["voice_rus.idx", "voice_rus2.idx", "voiceDLC1_rus.idx"];
const ENG: [&str; 3] = ["voiceDLC1_2_eng", "voiceDLC2_eng2", "BUILD_voiceDLC4_eng"];
const EXTS: [&str; 3] = [".idx", ".idx.database", ".lipsync"];

fn is_tok(b: u8) -> bool { b.is_ascii_alphanumeric() || b == b'_' }
fn tok_end(d: &[u8], i: usize) -> usize { let mut j = i; while j < d.len() && is_tok(d[j]) { j += 1; } j }

// cue names from an index: \CUE\file.OGG
fn cues(path: &Path) -> HashSet<Vec<u8>> {
    let mut set = HashSet::new();
    let d = match fs::read(path) { Ok(d) => d, Err(_) => return set };
    let mut i = 0;
    while i < d.len() {
        if d[i] == b'\\' {
            let a = i + 1;
            let b = tok_end(&d, a);
            if b > a && b < d.len() && d[b] == b'\\' {
                let c = tok_end(&d, b + 1);
                if c + 4 <= d.len() && c > b + 1 && d[c..c + 4].eq_ignore_ascii_case(b".OGG") {
                    set.insert(d[a..b].to_ascii_uppercase());
                    i = c;
                    continue;
                }
            }
        }
        i += 1;
    }
    set
}

fn patch(d: &mut [u8], ov: &HashSet<Vec<u8>>) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < d.len() {
        if is_tok(d[i]) && (i == 0 || !is_tok(d[i - 1])) {
            let j = tok_end(d, i);
            if j < d.len() && (d[j] == b'\\' || d[j] == 0) && ov.contains(&d[i..j].to_ascii_uppercase()) {
                d[i] = if d[i] == b'Z' || d[i] == b'z' { b'Q' } else { b'Z' };
                n += 1;
            }
            i = j;
        } else { i += 1; }
    }
    n
}

fn patch_lipsync(d: &mut [u8], ov: &HashSet<Vec<u8>>) -> usize {
    if d.len() < 4 { return 0; }
    let cnt = u32::from_le_bytes([d[0], d[1], d[2], d[3]]) as usize;
    if 4 + cnt * 22 > d.len() { return patch(d, ov); }
    let mut n = 0;
    for k in 0..cnt {
        let s = 4 + k * 22;
        let end = (s..s + 16).find(|&i| d[i] == 0).unwrap_or(s + 16);
        if end > s && ov.contains(&d[s..end].to_ascii_uppercase()) {
            d[s] = if d[s] == b'Z' || d[s] == b'z' { b'Q' } else { b'Z' };
            n += 1;
        }
    }
    n
}

/// Returns the number of English lines hidden. `write` stores a changed file (with backup).
pub fn fix(game: &Path, write: &mut dyn FnMut(&Path, &[u8]) -> Result<(), String>) -> Result<usize, String> {
    let sd = game.join("SOUNDDATA");
    let mut rus = HashSet::new();
    for f in RUS { rus.extend(cues(&sd.join(f))); }
    if rus.is_empty() { return Err("русская озвучка не найдена".into()); }
    let mut total = 0;
    for base in ENG {
        if !sd.join(format!("{base}.idx")).exists() { continue; }
        for ext in EXTS {
            let p = sd.join(format!("{base}{ext}"));
            if !p.exists() { continue; }
            let mut d = fs::read(&p).map_err(|e| format!("не удалось прочитать {}: {e}", p.display()))?;
            let n = if ext == ".lipsync" { patch_lipsync(&mut d, &rus) } else { patch(&mut d, &rus) };
            if n == 0 { continue; }
            if ext == ".idx" { total += n; }
            write(&p, &d)?;
        }
    }
    Ok(total)
}
