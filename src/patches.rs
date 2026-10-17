// Binary patches for TwoWorlds2.exe / TwoWorlds2_DX10.exe (pattern based, applied to an unmodified exe).
use crate::blob;
use crate::pe::*;

#[derive(Clone, Default)]
pub struct BinOpts {
    pub input: bool, pub sprint: bool, pub sneak: bool, pub block: bool, pub saves: bool, pub quickload: bool,
    pub noblur: bool, pub borderless: bool, pub laa: bool, pub instant: bool,
    pub far_draw: bool, pub far_grass: bool, pub wpnfast: bool,
    pub sprint_mode: u8, pub sneak_mode: u8, pub sprint_vk: u8, pub sneak_vk: u8, pub ql_vk: u8,
}

/// Feature ids (as in the UI) that could not be applied, plus log lines.
pub struct Outcome { pub na: Vec<&'static str>, pub log: Vec<String>, pub changed: bool }

fn try_on(d: &mut Vec<u8>, f: impl FnOnce(&mut Vec<u8>) -> Result<(), String>) -> Result<(), String> {
    let mut t = d.clone();
    f(&mut t)?;
    *d = t;
    Ok(())
}

pub fn already_modified(d: &[u8]) -> bool {
    (match parse(d) { Ok(p) => p.sec(".tw2m").is_some() || p.sec(".tw2c").is_some() || p.sec(".tw2i").is_some(), Err(_) => false })
        || d.windows(4).any(|w| w == b"TWMF" || w == b"TWSP")
}

pub fn apply(d: &mut Vec<u8>, o: &BinOpts) -> Outcome {
    let mut out = Outcome { na: vec![], log: vec![], changed: false };
    let mut code = false;
    if o.noblur {
        match try_on(d, blur) { Ok(_) => code = true, Err(e) => { out.na.push("noblur"); out.log.push(format!("размытие: {e}")); } }
    }
    if o.borderless {
        match try_on(d, borderless) { Ok(_) => code = true, Err(e) => { out.na.push("borderless"); out.log.push(format!("окно: {e}")); } }
    }
    // gameplay (.tw2m) + save list / quick load (.tw2c)
    let mut want_m = o.input;
    let mut want_c = o.saves || o.quickload;
    loop {
        if !want_m && !want_c { break; }
        let mut t = d.clone();
        let r = sections(&mut t, o, want_m, want_c);
        match r {
            Ok(_) => { *d = t; code = true; break; }
            Err((which, e)) => {
                out.log.push(e);
                if which == 'c' {
                    want_c = false;
                    if o.saves { out.na.push("saves"); }
                    if o.quickload { out.na.push("quickload"); }
                } else {
                    want_m = false;
                    for f in ["sprint", "sneak", "block"] { out.na.push(f); }
                    if o.quickload && !out.na.contains(&"quickload") { out.na.push("quickload"); }
                    if !o.saves { want_c = false; }
                }
            }
        }
    }
    if o.instant {
        match try_on(d, instant) { Ok(_) => code = true, Err(e) => { out.na.push("instant"); out.log.push(format!("окна: {e}")); } }
    }
    if o.wpnfast {
        match try_on(d, wpnfast) { Ok(_) => code = true, Err(e) => { out.na.push("wpnfast"); out.log.push(format!("оружие: {e}")); } }
    }
    if o.far_draw {
        match try_on(d, far_draw) { Ok(_) => code = true, Err(e) => { out.na.push("draw"); out.log.push(format!("дальность: {e}")); } }
    }
    if o.far_grass {
        match try_on(d, far_grass) { Ok(_) => code = true, Err(e) => { out.na.push("grass"); out.log.push(format!("трава: {e}")); } }
    }
    if let Ok(p) = parse(d) {
        let pe = p.pe;
        if o.laa { let c = u16at(d, pe + 22); put16(d, pe + 22, c | 0x20); out.changed = true; }
        if code {
            let c = u16at(d, pe + 24 + 70);
            put16(d, pe + 24 + 70, c & !0x40); // fixed load address: our code uses absolute addresses
            out.changed = true;
        }
    }
    out
}

fn sections(t: &mut Vec<u8>, o: &BinOpts, want_m: bool, want_c: bool) -> Result<(), (char, String)> {
    let m = if want_m { Some(add_section(t, ".tw2m", 0x1000).map_err(|e| ('m', e))?) } else { None };
    let c = if want_c { Some(add_section(t, ".tw2c", 0x2000).map_err(|e| ('c', e))?) } else { None };
    if let Some((cva, craw)) = c {
        let vkp = if want_m && o.quickload { m.unwrap().0 + 0x604 } else { cva + 0x1ff0 };
        saves(t, cva, craw, vkp, o.saves).map_err(|e| ('c', format!("сохранения: {e}")))?;
    }
    if let Some((mva, mraw)) = m {
        input(t, mva, mraw, c.map(|x| x.0 + 0x1f00), o).map_err(|e| ('m', format!("управление: {e}")))?;
    }
    Ok(())
}

// ---------------------------------------------------------------- instant windows: camera jumps to the inventory view at once
fn instant(d: &mut Vec<u8>) -> Result<(), String> {
    // camera update: reads the "hero window open" flags once per frame
    let m = one(d, "a1 ?? ?? ?? ?? 8b 0d ?? ?? ?? ?? 8b 35 ?? ?? ?? ?? 89 84 24 ?? ?? 00 00 89 8c 24 ?? ?? 00 00 3b c3 75 08 89 5c 24", "камера")?;
    let (fa, fb, fc) = (u32at(d, m + 1), u32at(d, m + 7), u32at(d, m + 13));
    let mut f = m;
    loop {
        if f + 0x200 < m || f < 8 { return Err("камера: начало".into()); }
        if d[f - 1] == 0xcc && d[f..f + 6] == [0x55, 0x8b, 0xec, 0x83, 0xe4, 0xf0] { break; }
        f -= 1;
    }
    // smoothing step used by the camera
    let dm = one(d, "83 79 14 00 74 68 f3 0f 10 19 0f 57 d2 0f 2f da f3 0f 10 64 24 04", "сглаживание")?;
    let (sva, sraw) = add_section(d, ".tw2i", 0x1000)?;
    let p = parse(d)?;
    let (fva, dva) = (p.o2v(f), p.o2v(dm));
    let v = sva + 0x800;
    let (sa, sb, sc, snap, active) = (v, v + 4, v + 8, v + 12, v + 16);
    let (lo, hi) = (fa & !0xfff, (fa & !0xfff) + 0x5000);
    let mut a = Asm::new(sva);
    a.e(&[b"\x50"]);
    a.e(&[b"\xa1", &le(fa), b"\x3b\x05", &le(sa)]); a.j(b"\x75", "chg");
    a.e(&[b"\xa1", &le(fb), b"\x3b\x05", &le(sb)]); a.j(b"\x75", "chg");
    a.e(&[b"\xa1", &le(fc), b"\x3b\x05", &le(sc)]); a.j(b"\x74", "same");
    a.l("chg");
    a.e(&[b"\xa1", &le(fa), b"\xa3", &le(sa), b"\xa1", &le(fb), b"\xa3", &le(sb), b"\xa1", &le(fc), b"\xa3", &le(sc)]);
    a.e(&[b"\xc7\x05", &le(snap), &le(3)]);
    a.l("same");
    a.e(&[b"\x31\xc0\x83\x3d", &le(snap), b"\x00"]); a.j(b"\x74", "st");
    a.e(&[b"\xff\x0d", &le(snap), b"\x40"]);
    a.l("st"); a.e(&[b"\xa3", &le(active), b"\x58"]);
    a.e(&[b"\x55\x8b\xec\x83\xe4\xf0\xe9"]); a.rel(fva + 6);
    let c1 = a.done()?;
    let mut b = Asm::new(sva + 0x100);
    b.e(&[b"\x83\x3d", &le(active), b"\x00"]); b.j(b"\x74", "o");
    b.e(&[b"\x81\xf9", &le(lo)]); b.j(b"\x72", "o");
    b.e(&[b"\x81\xf9", &le(hi)]); b.j(b"\x73", "o");
    b.e(&[b"\xc7\x44\x24\x04", &le(0x447a_0000)]); // step = 1000: smoothing reaches its target at once
    b.l("o"); b.e(&[b"\x83\x79\x14\x00\x0f\x84"]); b.rel(dva + 0x6e);
    b.e(&[b"\xe9"]); b.rel(dva + 6);
    let c2 = b.done()?;
    let mut blob = vec![0xccu8; 0x1000];
    blob[..c1.len()].copy_from_slice(&c1);
    blob[0x100..0x100 + c2.len()].copy_from_slice(&c2);
    blob[0x800..0x820].copy_from_slice(&[0u8; 0x20]);
    blob[0xff0..0xff4].copy_from_slice(b"TWIW");
    d[sraw..sraw + 0x1000].copy_from_slice(&blob);
    d[f] = 0xe9; d[f + 1..f + 5].copy_from_slice(&rel32(fva + 5, sva)); d[f + 5] = 0x90;
    d[dm] = 0xe9; d[dm + 1..dm + 5].copy_from_slice(&rel32(dva + 5, sva + 0x100)); d[dm + 5] = 0x90;
    // the camera ignores the open hero windows: it stays where it is, the window opens over the game
    d[m..m + 17].copy_from_slice(&[0x31, 0xc0, 0x90, 0x90, 0x90, 0x31, 0xc9, 0x90, 0x90, 0x90, 0x90, 0x31, 0xf6, 0x90, 0x90, 0x90, 0x90]);
    // background loader: no 50 ms nap between files (window elements loaded one per nap)
    if let Ok(l) = one(d, "85 c0 75 04 6a 32 ff d6 8b 15", "загрузчик") { d[l + 5] = 1; }
    // hero window: the game HUD (mini-map, health, mana) is not hidden while it is open
    // two dialogs share this tail; the hero window is the one whose preceding call jumps forward
    let hv: Vec<usize> = Pat::new("e8 ?? ?? ?? ?? 8b c8 e8 ?? ?? ?? ?? 6a 01 e8 ?? ?? ?? ?? 83 c4 04 8b c6 5e c3").all(d)
        .into_iter().filter(|&a| (u32at(d, a + 8) as i32) > 0).collect();
    if let [h] = hv[..] {
        for i in 14..19 { d[h + i] = 0x90; }
    }
    Ok(())
}

// ---------------------------------------------------------------- faster weapon draw / put away (.tw2w)
// while the hero switches the weapon: animation crossfades run 2x faster and the draw/put-away animation plays at 1.6x
fn wpnfast(d: &mut Vec<u8>) -> Result<(), String> {
    let ho = first(d, "53 8b 1d ?? ?? ?? ?? 55 56 57 85 db 74 ?? f6 83 f8 00 00 00 80", 0, "герой")?;
    let hero = u32at(d, ho + 3);
    let t1 = one(d, "f3 0f 10 46 24 f3 0f 5c c1 0f 57 c9 f3 0f 11 46 24 f3 0f 10 46 24", "смена 1")?;
    let t4 = one(d, "f3 0f 59 44 24 40 f3 0f 58 00 0f 57 c9 0f 2f c8 77 0d", "смена 2")?;
    let hf = one(d, "8b 86 d0 00 00 00 3b c3 74 23 83 f8 01 74 1e 83 f8 02 74 19 83 f8 03 74 14 8b 86 d4 00 00 00 83 f8 09", "герой кадр")?;
    if hf < 198 || d[hf - 198..hf - 192] != [0x55, 0x8b, 0xec, 0x83, 0xe4, 0xf0] || d[hf - 199] != 0xcc { return Err("герой кадр: начало".into()); }
    let hf = hf - 198;
    let (sva, sraw) = add_section(d, ".tw2w", 0x1000)?;
    let p = parse(d)?;
    let (t1va, t4va, hfva) = (p.o2v(t1), p.o2v(t4), p.o2v(hf));
    let (k2, k16, k1, flag) = (sva + 0x800, sva + 0x804, sva + 0x808, sva + 0x80c);
    // crossfade on the first track: weight -= step  ->  step * 2 for the hero's animator
    let mut a = Asm::new(sva);
    a.e(&[b"\x50\xa1", &le(hero), b"\x85\xc0"]); a.j(b"\x74", "o");
    a.e(&[b"\xf6\x80\xf8\x00\x00\x00\x20"]); a.j(b"\x74", "o");
    a.e(&[b"\x3b\xb0\xfc\x03\x00\x00"]); a.j(b"\x75", "o");
    a.e(&[b"\xf3\x0f\x59\x0d", &le(k2)]);
    a.l("o"); a.e(&[b"\x58\xf3\x0f\x10\x46\x24\xf3\x0f\x5c\xc1\xe9"]); a.rel(t1va + 9);
    let c1 = a.done()?;
    // crossfade on the second track: weight += rate * dt  ->  rate * dt * 2 for the hero's track
    let mut b = Asm::new(sva + 0x100);
    b.e(&[b"\xf3\x0f\x59\x44\x24\x40\x51\x8b\x0d", &le(hero), b"\x85\xc9"]); b.j(b"\x74", "o");
    b.e(&[b"\xf6\x81\xf8\x00\x00\x00\x20"]); b.j(b"\x74", "o");
    b.e(&[b"\x8b\x89\xfc\x03\x00\x00\x83\xc1\x30\x39\xc1"]); b.j(b"\x75", "o");
    b.e(&[b"\xf3\x0f\x59\x05", &le(k2)]);
    b.l("o"); b.e(&[b"\x59\xf3\x0f\x58\x00\xe9"]); b.rel(t4va + 10);
    let c2 = b.done()?;
    // every hero frame: draw/put-away animation (type 0xd / 0x13) or switching in progress -> track speed 1.6, else back to 1.0
    let mut c = Asm::new(sva + 0x200);
    c.e(&[b"\x50\x52\xa1", &le(hero), b"\x39\xc8"]); c.jn(b"\x0f\x85", "o");
    c.e(&[b"\x8b\x90\xfc\x03\x00\x00\x85\xd2"]); c.jn(b"\x0f\x84", "o");
    c.e(&[b"\xf6\x80\xf8\x00\x00\x00\x20"]); c.j(b"\x75", "on");
    c.e(&[b"\x8b\x42\x0c\x85\xc0"]); c.j(b"\x74", "off");
    c.e(&[b"\x8b\x80\x80\x00\x00\x00\x83\xf8\x0d"]); c.j(b"\x74", "on");
    c.e(&[b"\x83\xf8\x13"]); c.j(b"\x74", "on");
    c.l("off");
    c.e(&[b"\x83\x3d", &le(flag), b"\x00"]); c.j(b"\x74", "o");
    c.e(&[b"\xa1", &le(k1), b"\x89\x42\x58\xc7\x05", &le(flag), &le(0)]); c.j(b"\xeb", "o");
    c.l("on");
    c.e(&[b"\xa1", &le(k16), b"\x89\x42\x58\xc7\x05", &le(flag), &le(1)]);
    c.l("o"); c.e(&[b"\x5a\x58\x55\x8b\xec\x83\xe4\xf0\xe9"]); c.rel(hfva + 6);
    let c3 = c.done()?;
    let mut blob = vec![0xccu8; 0x1000];
    blob[..c1.len()].copy_from_slice(&c1);
    blob[0x100..0x100 + c2.len()].copy_from_slice(&c2);
    blob[0x200..0x200 + c3.len()].copy_from_slice(&c3);
    blob[0x800..0x810].copy_from_slice(&[&le(0x4000_0000)[..], &le(0x3fcc_cccd), &le(0x3f80_0000), &le(0)].concat());
    blob[0xff0..0xff4].copy_from_slice(b"TWWF");
    d[sraw..sraw + 0x1000].copy_from_slice(&blob);
    d[t1] = 0xe9; d[t1 + 1..t1 + 5].copy_from_slice(&rel32(t1va + 5, sva)); for i in 5..9 { d[t1 + i] = 0x90; }
    d[t4] = 0xe9; d[t4 + 1..t4 + 5].copy_from_slice(&rel32(t4va + 5, sva + 0x100)); for i in 5..10 { d[t4 + i] = 0x90; }
    d[hf] = 0xe9; d[hf + 1..hf + 5].copy_from_slice(&rel32(hfva + 5, sva + 0x200)); d[hf + 5] = 0x90;
    Ok(())
}

// ---------------------------------------------------------------- extreme draw distance / grass: game sliders ignored, always the engine maximum
fn far_draw(d: &mut Vec<u8>) -> Result<(), String> {
    // far plane = 600 + 600 * setting, clamped to [800, 16384] -> always 16384
    let a = one(d, "f3 0f 10 0d ?? ?? ?? ?? f3 0f 10 44 24 04 f3 0f 59 c1 f3 0f 58 c1 f3 0f 10 0d ?? ?? ?? ?? 0f 2f c8 f3 0f 11 05", "дальность")?;
    let st = u32at(d, a + 37);
    d[a..a + 11].copy_from_slice(&[&[0xc7, 0x05][..], &le(st), &le(0x4680_0000), &[0xc3]].concat());
    Ok(())
}
fn far_grass(d: &mut Vec<u8>) -> Result<(), String> {
    // grass distance factor = setting clamped to [0, 1] -> always 1
    // several [0, 1] setters look alike: take the one whose value multiplies the grass user distance
    let cand: Vec<usize> = Pat::new("0f 57 c9 f3 0f 10 44 24 04 0f 2f c8 77 0d f3 0f 10 0d ?? ?? ?? ?? 0f 2f c1 76 03 0f 28 c1 f3 0f 11 05 ?? ?? ?? ?? c3").all(d)
        .into_iter().filter(|&a| {
            let st = le(u32at(d, a + 34));
            let pat = format!("f3 0f 10 05 ?? ?? ?? ?? f3 0f 59 05 {:02x} {:02x} {:02x} {:02x} f3 0f 59 05", st[0], st[1], st[2], st[3]);
            Pat::new(&pat).first(d, 0).is_some()
        }).collect();
    if cand.len() != 1 { return Err(format!("трава: найдено {}", cand.len())); }
    let a = cand[0];
    let st = u32at(d, a + 34);
    d[a..a + 11].copy_from_slice(&[&[0xc7, 0x05][..], &le(st), &le(0x3f80_0000), &[0xc3]].concat());
    Ok(())
}

// ---------------------------------------------------------------- blur
fn blur(d: &mut Vec<u8>) -> Result<(), String> {
    let a = one(d, "f3 0f 10 86 d4 02 00 00 f3 0f 11 05 ?? ?? ?? ?? f3 0f 10 86 d8 02 00 00 f3 0f 11 05 ?? ?? ?? ?? f3 0f 10 86 dc 02 00 00 f3 0f 11 05 ?? ?? ?? ??", "blur")?;
    const NOP8: [u8; 8] = [0x0f, 0x1f, 0x84, 0, 0, 0, 0, 0];
    d[a + 8..a + 16].copy_from_slice(&NOP8);
    d[a + 40..a + 48].copy_from_slice(&NOP8);
    Ok(())
}

// ---------------------------------------------------------------- borderless window, no minimize on focus loss
fn borderless(d: &mut Vec<u8>) -> Result<(), String> {
    // window style: overlapped+caption -> popup
    let a = one(d, "68 00 00 ca 10 8d 54 24 18 52 c7 44 24 30 00 00 ca 10", "style1")?;
    put32(d, a + 1, 0x9000_0000); put32(d, a + 14, 0x9000_0000);
    let b = one(d, "bf 00 00 ca 10 89 44 24 20 57", "style2")?;
    put32(d, b + 1, 0x9000_0000);
    // fullscreen flag passed to window mode / device creation -> 0
    let w = one(d, "a1 ?? ?? ?? ?? 8b 0d ?? ?? ?? ?? 8b 15 ?? ?? ?? ?? 50 51 52 e8 ?? ?? ?? ?? a1 ?? ?? ?? ?? 8b 0d ?? ?? ?? ?? 8b 15 ?? ?? ?? ?? 50 a1 ?? ?? ?? ?? 51 8b 0d ?? ?? ?? ?? 52 50 51 e8", "windowed")?;
    if u32at(d, w + 1) != u32at(d, w + 26) { return Err("windowed: разные флаги".into()); }
    d[w..w + 5].copy_from_slice(&[0x31, 0xc0, 0x90, 0x90, 0x90]);
    d[w + 25..w + 30].copy_from_slice(&[0x31, 0xc0, 0x90, 0x90, 0x90]);
    let v = Pat::new("8b b4 24 ?? 00 00 00 3b f3 75 06 89 5c 24 10 33 ed").all(d);
    if v.len() == 1 { d[v[0]..v[0] + 7].copy_from_slice(&[0x31, 0xf6, 0x90, 0x90, 0x90, 0x90, 0x90]); }
    minimize(d)?;
    minimize2(d)?;
    activate(d)?;
    Ok(())
}

fn text_range(p: &Pe) -> (usize, usize) { let t = p.text(); (t.ro as usize, (t.ro + t.vs.min(t.rs)) as usize) }

fn minimize(d: &mut Vec<u8>) -> Result<(), String> {
    let p = parse(d)?;
    let imp = p.iat(d, &["SetWindowPos", "MoveWindow", "GetClientRect"]);
    let g = |n: &str| imp.get(n).copied().ok_or(format!("импорт {n}"));
    let (swp_i, mw_i, gcr) = (g("SetWindowPos")?, g("MoveWindow")?, g("GetClientRect")?);
    let (tf, tt) = text_range(&p);
    let cave = cc_run(d, tf, tt, 72).ok_or("нет места")? + 1;
    let cva = p.o2v(cave);
    let mut chk: Vec<u8> = vec![0x83, 0xec, 0x10, 0x54, 0xff, 0x74, 0x24, 0x1c, 0xff, 0x15];
    chk.extend_from_slice(&le(gcr));
    chk.extend_from_slice(&[0x8b, 0x44, 0x24, 0x08, 0x0b, 0x44, 0x24, 0x0c, 0x83, 0xc4, 0x10, 0x85, 0xc0, 0xc3]);
    let stub = |at: u32, iat: u32, retn: u16| -> Vec<u8> {
        let mut s = vec![0xe8]; s.extend_from_slice(&rel32(at + 5, cva));
        s.extend_from_slice(&[0x74, 0x06, 0xff, 0x25]); s.extend_from_slice(&le(iat));
        s.extend_from_slice(&[0x31, 0xc0, 0x40, 0xc2]); s.extend_from_slice(&retn.to_le_bytes()); s
    };
    let swp_va = cva + 28;
    let swp = stub(swp_va, swp_i, 0x1c);
    let mw_va = swp_va + swp.len() as u32;
    let mw = stub(mw_va, mw_i, 0x18);
    let mut blob = chk; blob.extend(swp); blob.extend(mw); blob.extend_from_slice(b"TWMF");
    d[cave..cave + blob.len()].copy_from_slice(&blob);
    let t = p.text();
    let (tro, trs) = (t.ro as usize, t.rs as usize);
    for (iat, sva) in [(swp_i, swp_va), (mw_i, mw_va)] {
        let pat = format!("ff 15 {}", hexs(iat));
        for j in Pat::new(&pat).find_in(d, tro, tro + trs) {
            let site = p.o2v(j);
            d[j] = 0xe8; d[j + 1..j + 5].copy_from_slice(&rel32(site + 5, sva)); d[j + 5] = 0x90;
        }
    }
    Ok(())
}

fn hexs(v: u32) -> String { le(v).iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ") }

fn hwnd_global(d: &[u8], p: &Pe) -> Result<u32, String> {
    let imp = p.iat(d, &["SetWindowLongA"]);
    let swl = *imp.get("SetWindowLongA").ok_or("импорт SetWindowLongA")?;
    let m = first(d, &format!("a1 ?? ?? ?? ?? 6a f0 50 ff 15 {}", hexs(swl)), 0, "hwnd")?;
    Ok(u32at(d, m + 1))
}

fn minimize2(d: &mut Vec<u8>) -> Result<(), String> {
    let p = parse(d)?;
    const FUNC: &str = "83 ec 10 53 8b 5c 24 18 55 8b 6c 24 24 57 8b 7c 24 24";
    let fo = first(d, FUNC, 0, "resize")?;
    let gcr = *p.iat(d, &["GetClientRect"]).get("GetClientRect").ok_or("импорт GetClientRect")?;
    let hwnd = hwnd_global(d, &p)?;
    let (tf, tt) = text_range(&p);
    let cave = cc_run(d, tf, tt, 50).ok_or("нет места")? + 1;
    let cva = p.o2v(cave); let fva = p.o2v(fo);
    let mut g: Vec<u8> = vec![0x83, 0xec, 0x10, 0x54, 0xff, 0x35];
    g.extend_from_slice(&le(hwnd)); g.extend_from_slice(&[0xff, 0x15]); g.extend_from_slice(&le(gcr));
    g.extend_from_slice(&[0x85, 0xc0, 0x74, 0x10, 0x8b, 0x44, 0x24, 0x08, 0x0b, 0x44, 0x24, 0x0c, 0x83, 0xc4, 0x10, 0x85, 0xc0, 0x75, 0x04, 0xc3, 0x83, 0xc4, 0x10]);
    let orig = d[fo..fo + 8].to_vec();
    g.extend_from_slice(&orig);
    let nx = cva + g.len() as u32 + 5;
    g.push(0xe9); g.extend_from_slice(&rel32(nx, fva + 8));
    d[cave..cave + g.len()].copy_from_slice(&g);
    d[fo] = 0xe9; d[fo + 1..fo + 5].copy_from_slice(&rel32(fva + 5, cva)); d[fo + 5..fo + 8].copy_from_slice(&[0x90; 3]);
    Ok(())
}

fn activate(d: &mut Vec<u8>) -> Result<(), String> {
    let v = Pat::new("e8 ?? ?? ?? ?? 66 85 ff 75 ?? 85 c0 74 ?? 6a 00 e8").all(d);
    if v.len() == 1 {
        let a = v[0] + 12;
        if let Some(m2) = Pat::new("85 c0 74 ?? 6a 01 e8 ?? ?? ?? ?? 8b 0d").find_in(d, a, a + 0x60).first() {
            let b = m2 + 2;
            d[a..a + 2].copy_from_slice(&[0x90, 0x90]);
            d[b..b + 2].copy_from_slice(&[0x90, 0x90]);
            return Ok(());
        }
    }
    // other layout (DX9): show/restore or minimize the window ourselves
    let p = parse(d)?;
    let so = one(d, "8b cf 66 85 c9 0f 95 c0 8d 44 00 ff a3", "activate")?;
    let sva = p.o2v(so);
    let sw = *p.iat(d, &["ShowWindow"]).get("ShowWindow").ok_or("импорт ShowWindow")?;
    let hwnd = hwnd_global(d, &p)?;
    let fm = first(d, "6a 00 51 ff d6 c7 05 ?? ?? ?? ?? 01 00 00 00", 0, "flag")?;
    let flag = u32at(d, fm + 7);
    let (tf, tt) = text_range(&p);
    let co = cc_run(d, tf, tt, 58).ok_or("нет места")? + 1;
    let cva = p.o2v(co);
    let mut g: Vec<u8> = vec![0x8b, 0x35]; g.extend_from_slice(&le(sw));
    g.extend_from_slice(&[0x8b, 0x1d]); g.extend_from_slice(&le(hwnd));
    g.extend_from_slice(&[0x66, 0x85, 0xff, 0x74, 0x0e, 0x6a, 0x05, 0x53, 0xff, 0xd6, 0x6a, 0x09, 0x53, 0xff, 0xd6, 0x31, 0xc0, 0xeb, 0x08,
        0x6a, 0x06, 0x53, 0xff, 0xd6, 0x6a, 0x01, 0x58, 0xa3]);
    g.extend_from_slice(&le(flag));
    g.extend_from_slice(&[0x8b, 0xcf, 0x66, 0x85, 0xc9]);
    let nx = cva + g.len() as u32 + 5;
    g.push(0xe9); g.extend_from_slice(&rel32(nx, sva + 5));
    d[co..co + g.len()].copy_from_slice(&g);
    d[so] = 0xe9; d[so + 1..so + 5].copy_from_slice(&rel32(sva + 5, cva));
    Ok(())
}

// ---------------------------------------------------------------- save list + quick load (.tw2c)
fn saves(d: &mut Vec<u8>, secva: u32, secraw: usize, vkp: u32, redirect: bool) -> Result<(), String> {
    let p = parse(d)?;
    let rel_at = |d: &[u8], o: usize| p.o2v(o).wrapping_add(4).wrapping_add(i32at(d, o) as u32);
    let m = first(d, "55 8d be f4 01 00 00 57 52 50 c6 44 24 68 08 e8 ?? ?? ?? ??", 0, "список")?;
    let co = m + 15; let cva = p.o2v(co);
    let orig = rel_at(d, co + 1);
    let a = first(d, "68 ?? ?? ?? ?? 8d 45 0e 68 75 02 00 00 50 e8 ?? ?? ?? ??", 0, "alloc")?;
    let fileva = u32at(d, a + 1); let alloc = rel_at(d, a + 15);
    let im = p.iat(d, &["FileTimeToLocalFileTime", "FileTimeToSystemTime", "GetAsyncKeyState"]);
    let g = |n: &str| im.get(n).copied().ok_or(format!("импорт {n}"));
    let tm = first(d, "8b 15 ?? ?? ?? ?? 8b 92 e8 03 00 00", 0, "шаблоны")?;
    let tmpl = u32at(d, tm + 2);
    let p1 = "c7 44 24 10 ?? ?? ?? ?? 89 5c 24 14 89 5c 24 20 89 5c 24 1c 89 5c 24 18 89 5c 24 28 89 5c 24 34 89 5c 24 30 89 5c 24 2c c7 44 24 24 ?? ?? ?? ?? 8d 54 24 24 52 8d 44 24 14 50 53 53 c6 44 24 50 04 e8";
    let m1 = first(d, p1, 0, "m1")?;
    let (vt_list, vt_ids) = (u32at(d, m1 + 4), u32at(d, m1 + 44));
    let m2 = first(d, "8d 4c 24 24 c6 44 24 40 03 e8 ?? ?? ?? ?? 8d 4c 24 10 c6 44 24 40 02 e8", m1 + Pat::new(p1).len(), "m2")?;
    let dtor_ids = rel_at(d, m2 + 10); let dtor_list = rel_at(d, m2 + 24);
    let p3 = "39 15 ?? ?? ?? ?? 74 07 e8 ?? ?? ?? ?? eb 11 52 52 52 52 8b 54 24 48 52 e8";
    let m3 = first(d, p3, 0, "m3")?;
    let ingame = u32at(d, m3 + 2);
    let m4 = first(d, "8b 44 24 10 83 78 09 00 74 10 8d 4c 24 10 57 51 e8", m3 + Pat::new(p3).len(), "m4")?;
    let f_load = rel_at(d, m4 + 17);
    let m5 = first(d, "83 3d ?? ?? ?? ?? 00 0f 85 ?? ?? ?? ?? 83 3d ?? ?? ?? ?? 00 0f 85 ?? ?? ?? ?? 83 3d ?? ?? ?? ?? 00 0f 85 ?? ?? ?? ?? a1 ?? ?? ?? ?? 83 78 09 00 0f 8f ?? ?? ?? ?? 8b 0d ?? ?? ?? ?? 83 79 09 00 0f 8f", 0, "m5")?;
    let cfg = [orig, alloc, g("FileTimeToLocalFileTime")?, g("FileTimeToSystemTime")?, fileva, tmpl, f_load, dtor_list, dtor_ids,
        vt_list, vt_ids, ingame, u32at(d, m5 + 2), u32at(d, m5 + 15), u32at(d, m5 + 28), u32at(d, m5 + 40), u32at(d, m5 + 56),
        g("GetAsyncKeyState")?, vkp];
    let mut b = vec![0xccu8; 0x2000];
    for (i, v) in cfg.iter().enumerate() { put32(&mut b, i * 4, *v); }
    b[0x80..0x80 + blob::SAVES.len()].copy_from_slice(blob::SAVES);
    for &r in blob::SAVES_REL.iter() {
        let o = 0x80 + r as usize; let v = u32at(&b, o); put32(&mut b, o, v.wrapping_add(secva));
    }
    b[0x1f00] = 0xe9; b[0x1f01..0x1f05].copy_from_slice(&rel32(secva + 0x1f05, secva + blob::TICK));
    b[0x1ff0..0x1ff8].copy_from_slice(&[0; 8]);
    d[secraw..secraw + 0x2000].copy_from_slice(&b);
    if redirect {
        d[co + 1..co + 5].copy_from_slice(&rel32(cva + 5, secva + blob::HOOK));
        // the list opens at its top (the quick save), not scrolled to the newest save
        if let Ok(s) = one(d, "83 7c 24 14 00 7c 1c 8b 86 ?? ?? 00 00 8b 4c 07 14 3b 4c 24 2c 7c 26 7f 0a", "выбор") { d[s + 7] = 0xeb; d[s + 8] = 0x34; }
    }
    Ok(())
}

// ---------------------------------------------------------------- sprint / sneak / block / mouse (.tw2m)
#[allow(clippy::too_many_arguments)]
fn key(a: &mut Asm, gaks: u32, cfg: u32, cfgoff: u32, mode_off: u32, prev: u32, tog: u32, outreg: (&[u8], &[u8]), h: &'static str, t: &'static str, dn: &'static str) {
    a.e(&[b"\x0f\xb6\x05", &le(cfg + cfgoff), b"\x50\xff\x15", &le(gaks)]);
    a.e(&[b"\x31\xc9\xf6\xc4\x80\x0f\x95\xc1"]);
    a.e(&[b"\x8b\x15", &le(prev), b"\x89\x0d", &le(prev)]);
    a.e(&[b"\x80\x3d", &le(cfg + mode_off), b"\x02"]); a.j(b"\x75", h);
    a.e(&[b"\x85\xc9"]); a.j(b"\x74", t); a.e(&[b"\x85\xd2"]); a.j(b"\x75", t);
    a.e(&[b"\x83\x35", &le(tog), b"\x01"]);
    a.l(t); a.e(&[outreg.0, &le(tog)]); a.j(b"\xeb", dn);
    a.l(h); a.e(&[outreg.1]);
    a.l(dn);
}

fn input(d: &mut Vec<u8>, secva: u32, secraw: usize, tick: Option<u32>, o: &BinOpts) -> Result<(), String> {
    let p = parse(d)?;
    let im = p.iat(d, &["GetAsyncKeyState", "timeGetTime"]);
    let gaks = *im.get("GetAsyncKeyState").ok_or("импорт GetAsyncKeyState")?;
    let tgt = *im.get("timeGetTime").ok_or("импорт timeGetTime")?;
    let hook_o = first(d, "39 3d ?? ?? ?? ?? 0f 84 ?? ?? 00 00 39 7c 24 10 75 ?? e8 ?? ?? ?? ?? 85 c0 74 ?? 39 3d ?? ?? ?? ?? eb ?? 68 df 00 00 00 e8", 0, "hook")?;
    let bee8 = u32at(d, hook_o + 2); let hook_va = p.o2v(hook_o);
    let rel = Pat::new("39 3d ?? ?? ?? ?? 0f 84 ?? ?? 00 00 39 7c 24 10 75 ?? e8 ?? ?? ?? ?? 85 c0 74 ?? 39 3d ?? ?? ?? ?? eb ?? 6a 12 e8").find_in(d, hook_o, hook_o + 0x700);
    if rel.len() != 3 { return Err(format!("клавиши: найдено {}", rel.len())); }
    let (bec8, becc, be98) = (u32at(d, rel[0] + 2), u32at(d, rel[1] + 2), u32at(d, rel[2] + 2));
    let sneak_rel_o = rel[0] + 35; let sprint_rel_o = rel[2] + 35;
    let calls = Pat::new("6a 12 e8").find_in(d, hook_o, hook_o + 0x700);
    let sneak_auto_o = *calls.iter().find(|&&c| c > sneak_rel_o && c < rel[1] + 35).ok_or("sneak auto")?;
    let rmb_o = first(d, "83 3d ?? ?? ?? ?? 01 74 0b 8b cb e8 ?? ?? ?? ?? 85 c0 74 13 8b cb e8 ?? ?? ?? ?? 5f f7 d8 5e 1b c0 5d 83 e0 05", 0, "rmb")?;
    let bf18 = u32at(d, rmb_o + 2);
    let issue_o = first(d, "8b 44 24 0c 8b 4c 24 08 8b 54 24 04 6a 00 6a 00 50 51 52 e8 ?? ?? ?? ?? 83 c4 14 c3", 0, "issue")?;
    let issue = p.o2v(issue_o);
    let ho = first(d, "53 8b 1d ?? ?? ?? ?? 55 56 57 85 db 74 ?? f6 83 f8 00 00 00 80", 0, "hero")?;
    let hero = u32at(d, ho + 3);
    let mv_o = first(d, &format!("55 8b ec 83 e4 f0 81 ec 84 00 00 00 53 8b 1d {} 56 57 85 db 74", hexs(hero)), 0, "move")?;
    let mv_va = p.o2v(mv_o);
    let disp_o = first(d, "56 e8 ?? ?? ?? ?? 8b 74 24 08 85 c0 74 ?? e8 ?? ?? ?? ?? 8b c8 e8 ?? ?? ?? ?? 85 c0 74 ?? 85 f6 75", 0, "dispatch")?;
    let disp_va = p.o2v(disp_o);
    let inner = disp_va.wrapping_add(6).wrapping_add(i32at(d, disp_o + 2) as u32);
    let ma = first(d, "8b 10 8b 52 28 57 8d 4c 24 28 51 8d 8c 24 64 01 00 00 51 6a 14 50 ff d2 3d 1e 00 07 80", 0, "мышь1")?;
    let sa_o = ma + 24; let sa_va = p.o2v(sa_o);
    let mb = first(d, "8b 41 24 ff d0 39 3d ?? ?? ?? ??", 0, "мышь2")?;
    let sb_o = mb + 5; let sb_va = p.o2v(sb_o);
    let cmpimm = u32at(d, sb_o + 2);
    let combat = one(d, "83 3d ?? ?? ?? ?? 01 74 0b 8b cd e8 ?? ?? ?? ?? 85 c0 74 13 8b cd e8 ?? ?? ?? ?? f7 d8 5f 1b c0 5e 83 e0 05", "бой ПКМ")?;
    let ctail = first(d, "5f 5e b8 12 00 00 00", combat, "бой ПКМ 2")?;
    if ctail - combat >= 0x40 { return Err("бой ПКМ 3".into()); }
    let blk = one(d, "83 3d ?? ?? ?? ?? 01 74 ?? 8b cf e8 ?? ?? ?? ?? 85 c0 75 ?? 8b cf e8 ?? ?? ?? ?? 85 c0 75 03 8d 68 2e", "блок")?;
    let rz = *Pat::new("5f 5e 5d [31 33] c0 5b c3").find_in(d, rmb_o, rmb_o + 0x60).first().ok_or("rmb tail")?;
    if rz - (rmb_o + 2) > 127 { return Err("rmb far".into()); }

    let cfg = secva + 0x600; let v = secva + 0x500;
    let (spp, spt, spw, snp, snt, snw) = (v, v + 4, v + 8, v + 12, v + 16, v + 20);
    let (rf, rb) = (v + 24, v + 28);
    let (rf2, rt, ra3) = (v + 32, v + 36, v + 40);
    let (blkv, hold, dly) = (v + 44, v + 48, v + 52);
    let (hold_ms, dly_ms) = (500u32, 0u32);
    let supp = v + 68;

    let mut a = Asm::new(secva);
    a.e(&[b"\x60"]);
    a.e(&[b"\x83\x3d", &le(rf), b"\x00"]); a.j(b"\x74", "norep");
    a.e(&[b"\x83\x3d", &le(be98), b"\x00"]); a.j(b"\x75", "norep");
    a.e(&[b"\x83\x3d", &le(bec8), b"\x00"]); a.j(b"\x75", "norep");
    a.e(&[b"\xc7\x05", &le(rf), &le(0), b"\x6a\x00\x6a\x01\xff\x35", &le(rb), b"\xe8"]); a.rel(disp_va); a.e(&[b"\x83\xc4\x0c"]);
    a.l("norep");
    a.e(&[b"\x83\x3d", &le(rf2), b"\x00"]); a.jn(b"\x0f\x84", "norep2");
    a.e(&[b"\x83\x3d", &le(be98), b"\x00"]); a.jn(b"\x0f\x85", "norep2");
    a.e(&[b"\x83\x3d", &le(bec8), b"\x00"]); a.jn(b"\x0f\x85", "norep2");
    a.e(&[b"\x83\x3d", &le(rt), b"\x14"]); a.j(b"\x74", "anok"); // weapon in/out: as soon as the sprint is over
    a.e(&[b"\x83\x3d", &le(bf18), b"\x01"]); a.jn(b"\x0f\x84", "norep2");
    a.e(&[b"\x8b\x0d", &le(hero), b"\x85\xc9"]); a.j(b"\x74", "anok");
    a.e(&[b"\x8b\x81\xd0\x00\x00\x00"]);
    for an in [1u32, 2, 3, 0xf0] { a.e(&[b"\x3d", &le(an)]); a.jn(b"\x0f\x84", "norep2"); }
    a.l("anok");
    a.e(&[b"\x83\x3d", &le(dly), b"\x00"]); a.j(b"\x74", "nodly");
    a.e(&[b"\xff\x15", &le(tgt), b"\x2b\x05", &le(dly)]); a.jn(b"\x0f\x88", "norep2");
    a.e(&[b"\xc7\x05", &le(dly), &le(0)]);
    a.l("nodly");
    a.e(&[b"\xc7\x05", &le(blkv), &le(0)]);
    a.e(&[b"\xc7\x05", &le(rf2), &le(0), b"\xff\x35", &le(ra3), b"\xff\x35", &le(rt), b"\x6a\x01\xe8"]); a.rel(secva + 0x9c0); a.e(&[b"\x83\xc4\x0c"]);
    a.e(&[b"\x83\x3d", &le(supp), b"\x02"]); a.j(b"\x75", "nsp2");
    a.e(&[b"\xc7\x05", &le(supp), &le(0)]); // weapon action done: the sprint may go on
    a.l("nsp2");
    a.e(&[b"\x83\x3d", &le(rt), b"\x12"]); a.j(b"\x75", "nohold");
    a.e(&[b"\xff\x15", &le(tgt), b"\x05", &le(hold_ms), b"\xa3", &le(hold)]);
    a.l("nohold");
    a.l("norep2");
    a.e(&[b"\x83\x3d", &le(blkv), b"\x00"]); a.j(b"\x74", "nblk");
    a.e(&[b"\x6a\x02\xff\x15", &le(gaks), b"\xf6\xc4\x80"]); a.j(b"\x75", "nblk");
    a.e(&[b"\xc7\x05", &le(blkv), &le(0), b"\xc7\x05", &le(rf2), &le(0)]);
    a.l("nblk");
    key(&mut a, gaks, cfg, 2, 0, spp, spt, (b"\x8b\x1d", b"\x89\xcb"), "sph", "spt", "spd");
    // sprint stays off after an action interrupted it, until the sprint key is let go
    a.e(&[b"\x83\x3d", &le(supp), b"\x00"]); a.j(b"\x74", "nsup");
    a.e(&[b"\x83\x3d", &le(supp), b"\x02"]); a.j(b"\x74", "ksup"); // weapon action pending: sprint paused
    a.e(&[b"\x83\x3d", &le(supp), b"\x03"]); a.j(b"\x75", "nsb");  // block held: sprint paused until it is let go
    a.e(&[b"\x51\x6a\x02\xff\x15", &le(gaks), b"\x59\xf6\xc4\x80"]); a.j(b"\x75", "ksup");
    a.e(&[b"\xc7\x05", &le(supp), &le(0)]); a.j(b"\xeb", "nsup");
    a.l("nsb");
    a.e(&[b"\x85\xc9"]); a.j(b"\x75", "ksup");
    a.e(&[b"\xc7\x05", &le(supp), &le(0)]); a.j(b"\xeb", "nsup");
    a.l("ksup"); a.e(&[b"\x31\xdb"]);
    a.l("nsup");
    key(&mut a, gaks, cfg, 3, 1, snp, snt, (b"\x8b\x2d", b"\x89\xcd"), "snh", "snt", "snd");
    // no sneaking with a weapon in hand
    a.e(&[b"\x8b\x0d", &le(hero), b"\x85\xc9"]); a.j(b"\x74", "nsnw");
    a.e(&[b"\xf6\x81\xf8\x00\x00\x00\x04"]); a.j(b"\x74", "nsnw");
    a.e(&[b"\x31\xed\xc7\x05", &le(snt), &le(0)]);
    a.l("nsnw");
    a.e(&[b"\x85\xdb"]); a.j(b"\x74", "mouse");
    a.e(&[b"\x31\xed\xc7\x05", &le(snt), &le(0)]);
    a.l("mouse");
    // left button always stops the sprint; right button only with a weapon drawn (without one it does nothing)
    a.e(&[b"\x8b\x0d", &le(hero), b"\x85\xc9"]); a.j(b"\x74", "nomouse");
    a.e(&[b"\xf6\x81\xf8\x00\x00\x00\x04"]); a.j(b"\x74", "nomouse"); // only with a weapon in hand
    a.e(&[b"\x6a\x01\xff\x15", &le(gaks), b"\xf6\xc4\x80"]); a.j(b"\x75", "mcan");
    a.e(&[b"\x6a\x02\xff\x15", &le(gaks), b"\xf6\xc4\x80"]); a.j(b"\x74", "nomouse");
    a.l("mcan");
    a.e(&[b"\x31\xdb\x31\xed\xc7\x05", &le(spt), &le(0), b"\xc7\x05", &le(snt), &le(0)]);
    a.l("nomouse");
    a.e(&[b"\x8b\x0d", &le(hero), b"\x85\xc9"]); a.j(b"\x74", "nomove");
    a.e(&[b"\x83\x3d", &le(bf18), b"\x01"]); a.j(b"\x74", "mv");
    a.e(&[b"\x83\xb9\x38\x01\x00\x00\x00"]); a.j(b"\x75", "mv");
    a.l("nomove"); a.e(&[b"\x31\xdb\xc7\x05", &le(spt), &le(0)]);
    a.l("mv"); a.e(&[b"\x89\x1d", &le(spw), b"\x89\x2d", &le(snw)]);
    a.e(&[b"\x83\x7c\x24\x30\x00"]); a.j(b"\x75", "done");
    a.e(&[b"\x8b\x0d", &le(hero), b"\x85\xc9"]); a.j(b"\x74", "done");
    a.e(&[b"\x85\xdb"]); a.j(b"\x74", "s2");
    a.e(&[b"\x83\x3d", &le(be98), b"\x00"]); a.j(b"\x75", "s2");
    a.e(&[b"\xf6\x81\xf8\x00\x00\x00\x80"]); a.j(b"\x75", "s2");
    a.e(&[b"\x6a\x00\x6a\x05\x6a\x01\xe8"]); a.rel(issue); a.e(&[b"\x83\xc4\x0c"]);
    a.l("s2");
    a.e(&[b"\x85\xed"]); a.j(b"\x74", "done");
    a.e(&[b"\x8b\x0d", &le(hero)]);
    a.e(&[b"\x83\x3d", &le(bec8), b"\x00"]); a.j(b"\x75", "done");
    a.e(&[b"\xf6\x81\xf8\x00\x00\x00\x80"]); a.j(b"\x75", "done");
    a.e(&[b"\x6a\x00\x6a\x2e\x6a\x01\xe8"]); a.rel(issue); a.e(&[b"\x83\xc4\x0c"]);
    a.l("done"); a.e(&[b"\x61\x39\x3d", &le(bee8), b"\xe9"]); a.rel(hook_va + 6);
    let code = a.done()?;
    if code.len() >= 0x400 { return Err("code size".into()); }

    let mut blob = vec![0xccu8; 0x1000];
    blob[..code.len()].copy_from_slice(&code);
    blob[0x400] = 0xa1; put32(&mut blob, 0x401, spw); blob[0x405] = 0xc3;
    blob[0x410] = 0xa1; put32(&mut blob, 0x411, snw); blob[0x415] = 0xc3;
    blob[0x500..0x580].copy_from_slice(&[0u8; 0x80]);
    blob[0x600..0x605].copy_from_slice(&[o.sprint_mode, o.sneak_mode, o.sprint_vk, o.sneak_vk, o.ql_vk]);
    blob[0x700..0x704].copy_from_slice(b"TWIN");

    let mut b = Asm::new(secva + 0x800);
    b.e(&[b"\x83\x7c\x24\x08\x01"]); b.j(b"\x75", "o");
    b.e(&[b"\x83\x7c\x24\x0c\x00"]); b.j(b"\x75", "o");
    b.e(&[b"\x83\x3d", &le(be98), b"\x00"]); b.j(b"\x75", "s");
    b.e(&[b"\x83\x3d", &le(bec8), b"\x00"]); b.j(b"\x74", "o");
    b.l("s"); b.e(&[b"\x8b\x44\x24\x04\xa3", &le(rb), b"\xc7\x05", &le(rf), &le(1), b"\xb8\x01\x00\x00\x00\xc3"]);
    b.l("o"); b.e(&[b"\x56\xe8"]); b.rel(inner); b.e(&[b"\xe9"]); b.rel(disp_va + 6);
    let bc = b.done()?; if bc.len() > 0x100 { return Err("b size".into()); }
    blob[0x800..0x800 + bc.len()].copy_from_slice(&bc);
    put32(&mut blob, 0x7f0, inner);

    let mut c2 = Asm::new(secva + 0xd00);
    c2.e(&[b"\x83\x7c\x24\x04\x01"]); c2.jn(b"\x0f\x85", "o");
    c2.e(&[b"\x8b\x44\x24\x08\x83\xf8\x05"]); c2.jn(b"\x0f\x84", "o");
    // sneaking with a weapon in hand breaks putting it away: ignore sneak then
    if o.sneak {
        c2.e(&[b"\x83\xf8\x2e"]); c2.j(b"\x74", "snk");
        c2.e(&[b"\x83\xf8\x4e"]); c2.j(b"\x75", "nsnk");
        c2.l("snk");
        c2.e(&[b"\x8b\x0d", &le(hero), b"\x85\xc9"]); c2.j(b"\x74", "nsnk");
        c2.e(&[b"\xf6\x81\xf8\x00\x00\x00\x04"]); c2.j(b"\x74", "nsnk");
        c2.e(&[b"\xb8\x01\x00\x00\x00\xc3"]);
        c2.l("nsnk");
    }
    c2.e(&[b"\x83\xf8\x2e"]); c2.jn(b"\x0f\x84", "o");
    c2.e(&[b"\x83\xf8\x02"]); c2.jn(b"\x0f\x84", "o"); // jump goes through at once, also from a sprint
    c2.e(&[b"\x83\xf8\x12"]); c2.j(b"\x75", "nb");
    // right button without a weapon in hand: ignore it (it only stopped the hero; sneaking is on its own key)
    if o.sneak {
        c2.e(&[b"\x8b\x0d", &le(hero), b"\x85\xc9"]); c2.j(b"\x74", "armd");
        c2.e(&[b"\xf6\x81\xf8\x00\x00\x00\x04"]); c2.j(b"\x75", "armd");
        c2.e(&[b"\xb8\x01\x00\x00\x00\xc3"]);
        c2.l("armd");
    }
    // instant block option off: right button goes the usual way
    if !o.block { c2.j(b"\xeb", "nb"); }
    c2.e(&[b"\x8b\x0d", &le(hero), b"\x85\xc9"]); c2.j(b"\x74", "nb");
    c2.e(&[b"\x83\x3d", &le(bf18), b"\x01"]); c2.j(b"\x74", "bm");
    c2.e(&[b"\x83\xb9\x38\x01\x00\x00\x00"]); c2.j(b"\x74", "nb");
    c2.l("bm"); c2.e(&[b"\xc7\x05", &le(blkv), &le(1)]); c2.j(b"\xeb", "s");
    c2.l("nb");
    c2.e(&[b"\x83\x3d", &le(be98), b"\x00"]); c2.j(b"\x75", "s");
    c2.e(&[b"\x83\x3d", &le(bec8), b"\x00"]); c2.jn(b"\x0f\x84", "o");
    c2.l("s");
    c2.e(&[b"\x83\x3d", &le(be98), b"\x00"]); c2.j(b"\x74", "nd");
    // an action pressed during a sprint ends the sprint; it runs right after.
    // weapon in/out: the sprint goes on afterwards if its key is still held (supp=2 until the action ran)
    c2.e(&[b"\x83\xf8\x14"]); c2.j(b"\x75", "nwp0");
    c2.e(&[b"\xc7\x05", &le(supp), &le(2)]); c2.j(b"\xeb", "swp");
    c2.l("nwp0");
    // block: the sprint goes on once the block button is let go (supp=3), if the sprint key is still held
    c2.e(&[b"\x83\xf8\x12"]); c2.j(b"\x75", "nwp");
    c2.e(&[b"\xc7\x05", &le(supp), &le(3)]); c2.j(b"\xeb", "swp");
    c2.l("nwp"); c2.e(&[b"\xc7\x05", &le(spt), &le(0), b"\xc7\x05", &le(supp), &le(1)]);
    c2.l("swp");
    c2.e(&[b"\x50\xff\x15", &le(tgt), b"\x05", &le(dly_ms), b"\xa3", &le(dly), b"\x58"]);
    c2.l("nd"); c2.e(&[b"\xa3", &le(rt), b"\x8b\x44\x24\x0c\xa3", &le(ra3), b"\xc7\x05", &le(rf2), &le(1), b"\xb8\x01\x00\x00\x00\xc3"]);
    c2.l("o"); c2.e(&[b"\x8b\x44\x24\x0c\x8b\x4c\x24\x08\xe9"]); c2.rel(issue + 8);
    let cc = c2.done()?; if cc.len() > 0x2f0 { return Err("c2 size".into()); }
    blob[0xd00..0xd00 + cc.len()].copy_from_slice(&cc);

    let mut c3 = Asm::new(secva + 0x900);
    c3.e(&[b"\x83\x3d", &le(blkv), b"\x00"]); c3.j(b"\x75", "z");
    c3.e(&[b"\x83\x3d", &le(hold), b"\x00"]); c3.j(b"\x74", "go");
    c3.e(&[b"\x51\x52"]);
    c3.e(&[b"\x83\x3d", &le(becc), b"\x00"]); c3.j(b"\x74", "clr");
    c3.e(&[b"\xa1", &le(hero), b"\x85\xc0"]); c3.j(b"\x74", "clr");
    c3.e(&[b"\x8b\x80\xd0\x00\x00\x00"]);
    c3.e(&[b"\x83\xf8\x16"]); c3.j(b"\x74", "clr");
    c3.e(&[b"\x3d", &le(0x28c)]); c3.j(b"\x74", "clr");
    c3.e(&[b"\xff\x15", &le(tgt), b"\x2b\x05", &le(hold)]); c3.j(b"\x79", "clr");
    c3.e(&[b"\x5a\x59"]); c3.j(b"\xeb", "z");
    c3.l("clr"); c3.e(&[b"\xc7\x05", &le(hold), &le(0), b"\x5a\x59"]);
    c3.l("go");
    c3.e(&[b"\x55\x8b\xec\x83\xe4\xf0\xe9"]); c3.rel(mv_va + 6);
    c3.l("z"); c3.e(&[b"\x31\xc0\xc3"]);
    let c3b = c3.done()?; if c3b.len() > 0xc0 { return Err("c3 size".into()); }
    blob[0x900..0x900 + c3b.len()].copy_from_slice(&c3b);

    let mut c4 = Asm::new(secva + 0x9c0);
    c4.e(&[b"\x8b\x44\x24\x0c\x8b\x4c\x24\x08\xe9"]); c4.rel(issue + 8);
    let c4b = c4.done()?;
    blob[0x9c0..0x9c0 + c4b.len()].copy_from_slice(&c4b);

    // mouse: LMB press during sprint is held back until the sprint ends, then delivered
    let (mp, mu, ms) = (v + 56, v + 60, v + 64);
    put32(&mut blob, 0x7e8, cmpimm);
    let mut m = Asm::new(secva + 0xb00);
    m.e(&[b"\x85\xc0"]); m.jn(b"\x0f\x85", "d0");
    m.e(&[b"\x60"]);
    if let Some(t) = tick { m.e(&[b"\xe8"]); m.rel(t); }
    if !o.sprint { m.jn(b"\xe9", "out"); }
    m.e(&[b"\x31\xd2"]);
    m.e(&[b"\x83\x3d", &le(be98), b"\x00"]); m.j(b"\x75", "spr");
    m.e(&[b"\xa1", &le(hero), b"\x85\xc0"]); m.j(b"\x74", "nspr");
    m.e(&[b"\x81\xb8\xd0\x00\x00\x00", &le(0xf0)]); m.j(b"\x75", "nspr");
    m.l("spr"); m.e(&[b"\x42"]);
    m.l("nspr");
    m.e(&[b"\x8b\x4c\x24\x44\x8d\xb4\x24\x7c\x01\x00\x00"]);
    m.l("lp"); m.e(&[b"\x85\xc9"]); m.j(b"\x74", "aft");
    m.e(&[b"\x8b\x46\x0c\xa3", &le(ms)]);
    m.e(&[b"\x83\x3e\x0c"]); m.j(b"\x75", "nx");
    m.e(&[b"\xf6\x46\x04\x80"]); m.j(b"\x74", "up");
    m.e(&[b"\x83\x3d", &le(mp), b"\x00"]); m.j(b"\x75", "nx");
    m.e(&[b"\x85\xd2"]); m.j(b"\x74", "nx");
    m.e(&[b"\xc7\x05", &le(mp), &le(1)]); m.j(b"\xeb", "kill");
    m.l("up"); m.e(&[b"\x83\x3d", &le(mp), b"\x00"]); m.j(b"\x74", "nx");
    m.e(&[b"\xc7\x05", &le(mu), &le(1)]);
    m.l("kill"); m.e(&[b"\xc7\x06", &le(0), b"\xc7\x46\x04", &le(0)]);
    m.l("nx"); m.e(&[b"\x83\xc6\x14\x49"]); m.j(b"\xeb", "lp");
    m.l("aft");
    m.e(&[b"\x83\x3d", &le(mp), b"\x00"]); m.jn(b"\x0f\x84", "out");
    m.e(&[b"\x85\xd2"]); m.jn(b"\x0f\x85", "out");
    m.e(&[b"\x8b\x4c\x24\x44\x81\xf9\xfe\x00\x00\x00"]); m.jn(b"\x0f\x83", "out");
    m.e(&[b"\x6b\xc1\x14\x8d\xb4\x04\x7c\x01\x00\x00"]);
    m.e(&[b"\x51\xff\x15", &le(tgt), b"\x59\x8b\x15", &le(ms)]);
    m.e(&[b"\xc7\x06\x0c\x00\x00\x00\xc7\x46\x04\x80\x00\x00\x00\x89\x46\x08\x89\x56\x0c\xc7\x46\x10\x00\x00\x00\x00\x41"]);
    m.e(&[b"\x83\x3d", &le(mu), b"\x00"]); m.j(b"\x74", "noup");
    m.e(&[b"\xc7\x46\x14\x0c\x00\x00\x00\xc7\x46\x18\x00\x00\x00\x00\x89\x46\x1c\x89\x56\x20\xc7\x46\x24\x00\x00\x00\x00\x41"]);
    m.l("noup");
    m.e(&[b"\x89\x4c\x24\x44\xc7\x05", &le(mp), &le(0), b"\xc7\x05", &le(mu), &le(0)]);
    m.l("out"); m.e(&[b"\x61"]);
    m.l("d0"); m.e(&[b"\x3d\x1e\x00\x07\x80\xe9"]); m.rel(sa_va + 5);
    let mc = m.done()?; if mc.len() >= 0x180 { return Err("mouse size".into()); }
    blob[0xb00..0xb00 + mc.len()].copy_from_slice(&mc);
    let mut mbb = Asm::new(secva + 0xc80);
    mbb.e(&[b"\x83\x3d", &le(mp), b"\x00"]); mbb.j(b"\x74", "o"); mbb.e(&[b"\xc6\x44\x24\x4c\x00"]);
    mbb.l("o"); mbb.e(&[b"\x39\x3d", &le(cmpimm), b"\xe9"]); mbb.rel(sb_va + 6);
    let mbc = mbb.done()?;
    blob[0xc80..0xc80 + mbc.len()].copy_from_slice(&mbc);

    // left button without a weapon in hand (action 0xdf = the kick): ignored completely
    let ko = one(d, "81 fe df 00 00 00 75 ?? 85 c9 74 ?? 83 b9 94 03 00 00 00 74", "пинок")?;
    let ko_va = p.o2v(ko);
    let k_other = ko_va + 8 + d[ko + 7] as u32;
    let mut k = Asm::new(secva + 0xcc0);
    k.e(&[b"\x81\xfe\xdf\x00\x00\x00"]); k.j(b"\x75", "oth");
    k.e(&[b"\xa1", &le(hero), b"\x85\xc0"]); k.j(b"\x74", "go");
    k.e(&[b"\xf6\x80\xf8\x00\x00\x00\x04"]); k.j(b"\x75", "go");
    k.e(&[b"\x5e\x5d\x5b\x5f\x31\xc0\xc3"]);
    k.l("go"); k.e(&[b"\xe9"]); k.rel(ko_va + 8);
    k.l("oth"); k.e(&[b"\xe9"]); k.rel(k_other);
    let kb = k.done()?; if kb.len() > 0x40 { return Err("kick size".into()); }
    blob[0xcc0..0xcc0 + kb.len()].copy_from_slice(&kb);
    if o.block {
        d[ko] = 0xe9; d[ko + 1..ko + 5].copy_from_slice(&rel32(ko_va + 5, secva + 0xcc0));
        for i in 5..8 { d[ko + i] = 0x90; }
    }

    // write section + redirect game code
    d[secraw..secraw + 0x1000].copy_from_slice(&blob);
    let jmp = |d: &mut Vec<u8>, o: usize, from_va: u32, to: u32, len: usize| {
        d[o] = 0xe9; d[o + 1..o + 5].copy_from_slice(&rel32(from_va + 5, to));
        for k in 5..len { d[o + k] = 0x90; }
    };
    // mouse hook always: it also drives the quick-load key
    jmp(d, sa_o, sa_va, secva + 0xb00, 5);
    // each control option changes only its own part of the game controls
    if o.sprint || o.sneak || o.block {
        jmp(d, sb_o, sb_va, secva + 0xc80, 6);
        jmp(d, issue_o, issue, secva + 0xd00, 8);
        jmp(d, disp_o, disp_va, secva + 0x800, 6);
        jmp(d, hook_o, hook_va, secva, 6);
    }
    if o.block { jmp(d, mv_o, mv_va, secva + 0x900, 6); }
    let mut rel_sites = vec![];
    if o.sprint { rel_sites.push((sprint_rel_o, secva + 0x400)); }
    if o.sneak { rel_sites.push((sneak_rel_o, secva + 0x410)); rel_sites.push((sneak_auto_o, secva + 0x410)); }
    for (site, to) in rel_sites {
        let cva = p.o2v(site + 2);
        d[site + 3..site + 7].copy_from_slice(&rel32(cva + 5, to));
    }
    // RMB outside combat: nothing (sneak is on its own key)
    if o.sneak { d[rmb_o] = 0xeb; d[rmb_o + 1] = (rz - (rmb_o + 2)) as u8; }
    if o.block {
        // RMB in combat: block; block while moving stays a block
        d[combat] = 0xeb; d[combat + 1] = (ctail - (combat + 2)) as u8;
        d[blk + 7..blk + 9].copy_from_slice(&[0x90, 0x90]);
        d[blk + 18..blk + 20].copy_from_slice(&[0x90, 0x90]);
    }
    Ok(())
}
