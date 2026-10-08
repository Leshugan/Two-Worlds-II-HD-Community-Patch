// Game search, checks, install / uninstall with backup.
use crate::patches::{self, BinOpts};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const BACKUP: &str = "_CommunityPatch_Backup";
const EXES: [(&str, &str); 2] = [("TwoWorlds2.exe", "DirectX 9"), ("TwoWorlds2_DX10.exe", "DirectX 10")];
const BEGIN: &str = "// === Two Worlds II HD Community Patch: begin ===";
const END: &str = "// === Two Worlds II HD Community Patch: end ===";

pub type Log<'a> = &'a dyn Fn(&str, &str);

// ------------------------------------------------------------------ search
fn is_game(p: &Path) -> bool { EXES.iter().any(|(e, _)| p.join(e).is_file()) }

fn reg_str(k: &winreg::RegKey, n: &str) -> Option<String> { k.get_value::<String, _>(n).ok() }

pub fn scan() -> Vec<(PathBuf, String)> {
    use winreg::enums::*;
    use winreg::RegKey;
    let mut c: Vec<(PathBuf, String)> = vec![];
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) {
        c.push((dir.clone(), "папка установщика".into()));
        if let Some(up) = dir.parent() { c.push((up.to_path_buf(), "папка установщика".into())); }
    }
    // Steam
    let mut steams: Vec<PathBuf> = vec![];
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(k) = hkcu.open_subkey(r"Software\Valve\Steam") { if let Some(s) = reg_str(&k, "SteamPath") { steams.push(PathBuf::from(s.replace('/', "\\"))); } }
    for p in [r"SOFTWARE\WOW6432Node\Valve\Steam", r"SOFTWARE\Valve\Steam"] {
        if let Ok(k) = hklm.open_subkey(p) { if let Some(s) = reg_str(&k, "InstallPath") { steams.push(PathBuf::from(s)); } }
    }
    steams.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    let mut libs: Vec<PathBuf> = vec![];
    for s in &steams {
        libs.push(s.clone());
        if let Ok(v) = fs::read_to_string(s.join(r"steamapps\libraryfolders.vdf")) {
            for line in v.lines() {
                let l = line.trim();
                if l.starts_with("\"path\"") {
                    let p: Vec<&str> = l.split('"').collect();
                    if p.len() >= 4 { libs.push(PathBuf::from(p[3].replace("\\\\", "\\"))); }
                }
            }
        }
    }
    for l in libs { c.push((l.join(r"steamapps\common\Two Worlds II"), "Steam".into())); }
    // GOG
    for root in [r"SOFTWARE\WOW6432Node\GOG.com\Games", r"SOFTWARE\GOG.com\Games"] {
        if let Ok(k) = hklm.open_subkey(root) {
            for name in k.enum_keys().flatten() {
                if let Ok(g) = k.open_subkey(&name) {
                    let gn = reg_str(&g, "gameName").or_else(|| reg_str(&g, "GAMENAME")).unwrap_or_default().to_lowercase();
                    if gn.contains("two worlds ii") || gn.contains("two worlds 2") {
                        if let Some(p) = reg_str(&g, "path").or_else(|| reg_str(&g, "PATH")) { c.push((PathBuf::from(p), "GOG".into())); }
                    }
                }
            }
        }
    }
    // uninstall entries (other stores / retail / repacks)
    for (hive, root) in [(&hklm, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"), (&hklm, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"), (&hkcu, r"Software\Microsoft\Windows\CurrentVersion\Uninstall")] {
        if let Ok(k) = hive.open_subkey(root) {
            for name in k.enum_keys().flatten() {
                if let Ok(g) = k.open_subkey(&name) {
                    let dn = reg_str(&g, "DisplayName").unwrap_or_default().to_lowercase();
                    if dn.contains("two worlds ii") || dn.contains("two worlds 2") {
                        if let Some(p) = reg_str(&g, "InstallLocation") { c.push((PathBuf::from(p.trim_matches('"')), "найдено".into())); }
                    }
                }
            }
        }
    }
    // common folders
    for d in b'C'..=b'Z' {
        let r = format!("{}:\\", d as char);
        if !Path::new(&r).exists() { continue; }
        for s in [r"Games\Two Worlds II", r"Games\Two Worlds II HD", r"Games\Two Worlds 2", r"GOG Games\Two Worlds II", r"Program Files (x86)\Two Worlds II",
                  r"Program Files\Two Worlds II", r"SteamLibrary\steamapps\common\Two Worlds II", r"Steam\steamapps\common\Two Worlds II", r"Two Worlds II"] {
            c.push((PathBuf::from(format!("{r}{s}")), "найдено".into()));
        }
    }
    let mut seen = HashSet::new();
    let mut out = vec![];
    for (p, mut l) in c {
        if !is_game(&p) { continue; }
        let key = fs::canonicalize(&p).unwrap_or(p.clone()).to_string_lossy().to_lowercase();
        if !seen.insert(key) { continue; }
        if l != "GOG" && p.to_string_lossy().to_lowercase().contains("steamapps") { l = "Steam".into(); }
        out.push((p, l));
    }
    out
}

/// Screen size and recommended FOV (the game widens the view on wide screens by itself, so wide screens need less).
pub fn screen_fov() -> (u32, u32, u32) {
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};
    let (w, h) = unsafe {
        let mut dm: DEVMODEW = std::mem::zeroed();
        dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        if EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut dm) != 0 { (dm.dmPelsWidth, dm.dmPelsHeight) } else { (1920, 1080) }
    };
    let a = w as f64 / h.max(1) as f64;
    let fov = if a >= 3.0 { 50 } else if a >= 2.2 { 55 } else if a >= 1.5 { 60 } else { 65 };
    (w, h, fov)
}

fn intro_files(game: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(game.join("Video")).map(|r| r.flatten().map(|e| e.path())
        .filter(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase().starts_with("intro")).unwrap_or(false)).collect()).unwrap_or_default();
    v.sort();
    v
}

pub fn refresh_rate() -> u32 {
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};
    unsafe {
        let mut dm: DEVMODEW = std::mem::zeroed();
        dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        if EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut dm) != 0 && dm.dmDisplayFrequency > 1 { dm.dmDisplayFrequency } else { 60 }
    }
}

// ------------------------------------------------------------------ backup manifest
pub struct Backup { dir: PathBuf, entries: Vec<Value>, seen: HashSet<String> }

impl Backup {
    fn manifest(game: &Path) -> PathBuf { game.join(BACKUP).join("manifest.json") }
    fn new(game: &Path) -> Result<Backup, String> {
        let dir = game.join(BACKUP);
        fs::create_dir_all(dir.join("files")).map_err(|e| format!("Не удалось создать папку резервной копии: {e}"))?;
        let b = Backup { dir, entries: vec![], seen: HashSet::new() };
        b.save()?;
        Ok(b)
    }
    fn save(&self) -> Result<(), String> {
        let v = json!({"version": 1, "entries": self.entries});
        fs::write(self.dir.join("manifest.json"), serde_json::to_vec_pretty(&v).unwrap()).map_err(|e| format!("Не удалось записать резервную копию: {e}"))
    }
    /// Remember the original state of a file before it is changed.
    fn touch(&mut self, p: &Path) -> Result<(), String> {
        let key = p.to_string_lossy().to_lowercase();
        if !self.seen.insert(key) { return Ok(()); }
        let ent = if p.exists() {
            let name = format!("{:03}_{}", self.entries.len(), p.file_name().unwrap().to_string_lossy());
            fs::copy(p, self.dir.join("files").join(&name)).map_err(|e| format!("Не удалось сохранить копию {}: {e}", p.display()))?;
            let ro = fs::metadata(p).map(|m| m.permissions().readonly()).unwrap_or(false);
            json!({"path": p, "bak": name, "ro": ro})
        } else { json!({"path": p, "bak": null}) };
        self.entries.push(ent);
        self.save()
    }
    fn byte(&mut self, p: &Path, off: usize, val: u8) -> Result<(), String> {
        self.entries.push(json!({"path": p, "byte": off, "val": val}));
        self.save()
    }
}

fn set_ro(p: &Path, ro: bool) {
    if let Ok(m) = fs::metadata(p) { let mut pm = m.permissions(); pm.set_readonly(ro); let _ = fs::set_permissions(p, pm); }
}

fn write(b: &mut Backup, p: &Path, data: &[u8]) -> Result<(), String> {
    b.touch(p)?;
    set_ro(p, false);
    if let Some(d) = p.parent() { let _ = fs::create_dir_all(d); }
    fs::write(p, data).map_err(|e| format!("Не удалось записать {}: {e}", p.display()))
}

pub fn is_installed(game: &Path) -> bool { Backup::manifest(game).is_file() }

fn manifest_entries(game: &Path) -> Vec<Value> {
    fs::read(Backup::manifest(game)).ok().and_then(|d| serde_json::from_slice::<Value>(&d).ok())
        .and_then(|v| v["entries"].as_array().cloned()).unwrap_or_default()
}

/// Original (pre-patch) content of a game file: from the backup if the patch is installed.
fn original(game: &Path, p: &Path) -> Option<Vec<u8>> {
    for e in manifest_entries(game) {
        if e["path"].as_str().map(|s| s.eq_ignore_ascii_case(&p.to_string_lossy())).unwrap_or(false) {
            return match e["bak"].as_str() { Some(b) => fs::read(game.join(BACKUP).join("files").join(b)).ok(), None => None };
        }
    }
    fs::read(p).ok()
}

pub fn uninstall(game: &Path, log: Log) -> Result<(), String> {
    if !is_installed(game) { return Err("Патч не установлен в этой папке.".into()); }
    let ents = manifest_entries(game);
    let files = game.join(BACKUP).join("files");
    let mut errs = 0;
    for e in ents.iter().rev() {
        let p = PathBuf::from(e["path"].as_str().unwrap_or_default());
        let short = p.strip_prefix(game).map(|x| x.display().to_string()).unwrap_or(p.display().to_string());
        let r: Result<(), String> = if let Some(off) = e["byte"].as_u64() {
            (|| { let mut d = fs::read(&p).map_err(|x| x.to_string())?; if (off as usize) < d.len() { d[off as usize] = e["val"].as_u64().unwrap_or(0) as u8; fs::write(&p, d).map_err(|x| x.to_string())?; } Ok(()) })()
        } else if let Some(b) = e["bak"].as_str() {
            set_ro(&p, false);
            fs::copy(files.join(b), &p).map(|_| ()).map_err(|x| x.to_string()).map(|_| { if e["ro"].as_bool().unwrap_or(false) { set_ro(&p, true); } })
        } else if p.exists() { set_ro(&p, false); fs::remove_file(&p).map_err(|x| x.to_string()) } else { Ok(()) };
        match r {
            Ok(_) => log(&format!("Восстановлено: {short}"), ""),
            Err(x) => { errs += 1; log(&format!("Не удалось восстановить {short}: {x}"), "bad"); }
        }
    }
    if errs > 0 { return Err(format!("Не восстановлено файлов: {errs}. Закройте игру и повторите. Резервная копия сохранена.")); }
    fs::remove_dir_all(game.join(BACKUP)).map_err(|e| format!("Не удалось удалить папку резервной копии: {e}"))?;
    Ok(())
}

// ------------------------------------------------------------------ check
fn all_opts() -> BinOpts {
    BinOpts { input: true, saves: true, quickload: true, noblur: true, borderless: true, laa: true, instant: true, far_draw: true, far_grass: true, wpnfast: true, sprint_mode: 1, sneak_mode: 2, sprint_vk: 0x10, sneak_vk: 0x43, ql_vk: 0x75 }
}
const BIN_FEATS: [&str; 8] = ["sprint", "sneak", "block", "quickload", "saves", "noblur", "borderless", "instant"];

fn rus_voice(game: &Path) -> bool { game.join(r"SOUNDDATA\voice_rus.idx").is_file() }
fn acf_path(game: &Path) -> Option<PathBuf> {
    let common = game.parent()?;
    if !common.file_name()?.to_string_lossy().eq_ignore_ascii_case("common") { return None; }
    let p = common.parent()?.join("appmanifest_7520.acf");
    if p.is_file() { Some(p) } else { None }
}

pub fn check(game: &Path) -> Value {
    if game.as_os_str().is_empty() { return json!({"ok": false, "msg": ""}); }
    let found: Vec<_> = EXES.iter().filter(|(e, _)| game.join(e).is_file()).collect();
    if found.is_empty() {
        return json!({"ok": false, "msg": "В этой папке нет TwoWorlds2.exe или TwoWorlds2_DX10.exe — укажите папку, где лежат эти файлы", "installed": false});
    }
    let mut na: Option<HashSet<&str>> = None;
    let mut foreign = false;
    for (e, _) in &found {
        let fail: HashSet<&str> = match original(game, &game.join(e)) {
            Some(mut d) if !patches::already_modified(&d) => patches::apply(&mut d, &all_opts()).na.into_iter().collect(),
            _ => { foreign = true; BIN_FEATS.iter().copied().collect() }
        };
        na = Some(match na { None => fail, Some(s) => s.intersection(&fail).copied().collect() });
    }
    let na: Vec<&str> = na.unwrap_or_default().into_iter().collect();
    let names: Vec<String> = found.iter().map(|(e, k)| format!("{e} ({k})")).collect();
    let ver = fs::read_to_string(game.join("VERSION")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s.len() < 16);
    let mut msg = format!("Игра найдена{}: {}", ver.as_ref().map(|v| format!(" (версия {v})")).unwrap_or_default(), names.join(", "));
    if let Some(v) = &ver { if v != "2.08" { msg += ". Патч делался под версию 2.08 — на другой версии часть функций может не работать"; } }
    if foreign { msg += ". Внимание: файлы игры уже изменены другим патчем — часть функций недоступна"; }
    json!({"ok": true, "msg": msg, "installed": is_installed(game), "steam": acf_path(game).is_some(), "voices": rus_voice(game), "intro": !intro_files(game).is_empty() || is_installed(game), "na": na})
}

// ------------------------------------------------------------------ install
pub struct Opts { pub v: Value }
impl Opts {
    fn on(&self, k: &str) -> bool { self.v[k].as_bool().unwrap_or(false) }
    fn num(&self, k: &str, d: u64) -> u64 { self.v[k].as_u64().filter(|&x| x > 0).unwrap_or(d) }
}

fn locked(p: &Path) -> bool { p.exists() && fs::OpenOptions::new().write(true).open(p).is_err() }

pub fn install(game: &Path, o: &Opts, log: Log) -> Result<(), String> {
    for (e, _) in EXES { if locked(&game.join(e)) { return Err("Игра запущена или файл занят. Закройте игру и повторите.".into()); } }
    if is_installed(game) {
        log("Найдена прошлая установка патча — возвращаю оригинальные файлы…", "skip");
        uninstall(game, &|_: &str, _: &str| {})?;
    }
    let mut b = Backup::new(game)?;
    log(&format!("Резервная копия: {}", game.join(BACKUP).display()), "");

    // executables
    let bo = BinOpts {
        input: o.on("sprint") || o.on("sneak") || o.on("block") || o.on("quickload"),
        saves: o.on("saves"), quickload: o.on("quickload"), noblur: o.on("noblur"), borderless: o.on("borderless"), laa: o.on("laa"), instant: o.on("instant"),
        far_draw: false, far_grass: o.on("grass") && o.num("grass_level", 2) >= 3, wpnfast: o.on("wpnfast"),
        sprint_mode: o.num("sprint_mode", 1) as u8, sneak_mode: o.num("sneak_mode", 2) as u8,
        sprint_vk: o.num("sprint_key", 0x10) as u8, sneak_vk: o.num("sneak_key", 0x43) as u8,
        ql_vk: if o.on("quickload") { o.num("quickload_key", 0x75) as u8 } else { 0 },
    };
    for (e, kind) in EXES {
        let p = game.join(e);
        if !p.is_file() { continue; }
        let mut d = fs::read(&p).map_err(|x| format!("Не удалось прочитать {e}: {x}"))?;
        if patches::already_modified(&d) { log(&format!("{e}: файл уже изменён другим патчем — изменения кода пропущены"), "skip"); continue; }
        let out = patches::apply(&mut d, &bo);
        let mut na = out.na.clone(); na.dedup();
        for f in &na { log(&format!("{e}: «{}» — недоступно для вашей версии", title(f)), "skip"); }
        if out.changed {
            write(&mut b, &p, &d)?;
            log(&format!("{e} ({kind}) — изменён"), "ok");
        }
    }

    // DXVK
    if o.on("dxvk") {
        for (n, data) in crate::DXVK { write(&mut b, &game.join(n), data)?; }
        let r = refresh_rate();
        let conf = format!("# Two Worlds II HD Community Patch — DXVK\r\n# Частота экрана {r} Гц: кадры ограничены чуть ниже неё для плавности\r\ndxvk.maxFrameRate = {}\r\n\r\n# Без этого DX10-версия работает на 60 Гц\r\ndxgi.forceRefreshRate = {r}\r\nd3d9.forceRefreshRate = {r}\r\n\r\n# Чёткие текстуры под углом\r\nd3d11.samplerAnisotropy = 16\r\nd3d9.samplerAnisotropy = 16\r\n", r.saturating_sub(3).max(30));
        write(&mut b, &game.join("dxvk.conf"), conf.as_bytes())?;
        log(&format!("DXVK установлен (экран {r} Гц)"), "ok");
    }

    // config lines
    let mut lines: Vec<String> = vec![];
    if o.on("camera") { lines.push("cam.damp 2".into()); lines.push("interface.Camera.MaxMouseMoveDxy 100000".into()); }
    if o.on("draw") {
        lines.push(format!("Settings.DrawDistanceMax {}", o.num("draw_level", 10)));
    }
    // no loadlevelmargin/unloadlevelmargin: they crash the game on teleport
    if o.on("horizon") { lines.push("Engine.HorizonClipAdjust 0.9".into()); }
    if o.on("shadows") {
        let (s0, s1) = if o.num("shadow_level", 2) >= 2 { (300, 1200) } else { (250, 900) };
        lines.push(format!("Engine.ShadowSelector0 {s0}")); lines.push(format!("Engine.ShadowSelector1 {s1}"));
    }
    if o.on("grass") {
        let (ud, fnc, end) = match o.num("grass_level", 2) { 3.. => ("8", 4000, 6000), 2 => ("3", 500, 1650), _ => ("2", 300, 1200) };
        lines.push(format!("Settings.DrawGrassDistanceMax {ud}")); lines.push(format!("Engine.GrassUserDistance {ud}"));
        lines.push(format!("Engine.GrassFadeFnc {fnc}")); lines.push(format!("Engine.GrassFadeEnd {end}"));
    }
    if o.on("fov") { lines.push(format!("Engine.FOV {}", o.num("fov_val", screen_fov().2 as u64).clamp(40, 90))); }
    if !lines.is_empty() {
        let p = game.join(r"parameters\autoexecgame2.con");
        let mut data = fs::read(&p).unwrap_or_default();
        if !data.is_empty() && !data.ends_with(b"\n") { data.extend_from_slice(b"\r\n"); }
        data.extend_from_slice(format!("\r\n{BEGIN}\r\n{}\r\n{END}\r\n", lines.join("\r\n")).as_bytes());
        write(&mut b, &p, &data)?;
        log("Настройки камеры и графики записаны", "ok");
    }

    // intro videos: logos always go; the story intro (the longest video) stays if the option is on
    let intros = intro_files(game);
    if !intros.is_empty() {
        let story = intros.iter().filter(|p| p.extension().map(|e| e.eq_ignore_ascii_case("wmv")).unwrap_or(false))
            .max_by_key(|p| fs::metadata(p).map(|m| m.len()).unwrap_or(0)).cloned();
        let keep_stem = story.as_ref().and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()));
        let mut n = 0;
        for p in &intros {
            let stem = p.file_stem().map(|s| s.to_string_lossy().to_lowercase());
            if o.on("intro") && stem.is_some() && stem == keep_stem { continue; }
            b.touch(p)?;
            set_ro(p, false);
            if fs::remove_file(p).is_ok() { n += 1; }
        }
        if n > 0 { log(if o.on("intro") { "Заставки с логотипами убраны, сюжетный ролик оставлен" } else { "Вступительные ролики убраны" }, "ok"); }
    }

    // distant trees: horizon shader without the overbright yellow-green foliage (DirectX 10)
    if o.on("lodtrees") {
        if game.join("TwoWorlds2_DX10.exe").is_file() { write(&mut b, &game.join(r"Shaders\horizon.DX0"), include_bytes!("../assets/horizon.DX0"))?; }
        if game.join("TwoWorlds2.exe").is_file() { write(&mut b, &game.join(r"Shaders\horizon.DX9"), include_bytes!("../assets/horizon.DX9"))?; }
        log("Лоды объектов: дальние деревья и зелень в цвет настоящих", "ok");
    }

    // voices
    if o.on("voices") {
        if rus_voice(game) {
            match crate::voices::fix(game, &mut |p: &Path, d: &[u8]| write(&mut b, p, d)) {
                Ok(0) => log("Озвучка: исправлять нечего", "skip"),
                Ok(n) => log(&format!("Озвучка: убрано английских реплик — {n}"), "ok"),
                Err(e) => log(&format!("Озвучка: {e}"), "bad"),
            }
        } else { log("Озвучка: русская озвучка не установлена — пропущено", "skip"); }
    }

    // loot animation
    if o.on("noloot") {
        let mut n = 0;
        if let Ok(la) = std::env::var("LOCALAPPDATA") {
            if let Ok(rd) = fs::read_dir(Path::new(&la).join(r"Two Worlds II\Players")) {
                for e in rd.flatten() {
                    let p = e.path().join("UserInfo.usr");
                    if let Ok(mut d) = fs::read(&p) {
                        if d.len() > 1646 && d[1646] == 1 { b.byte(&p, 1646, 1)?; d[1646] = 0; if fs::write(&p, &d).is_ok() { n += 1; } }
                        else if d.len() > 1646 && d[1646] == 0 { n += 1; }
                    }
                }
            }
        }
        if n > 0 { log("Анимация подбора предметов отключена", "ok"); } else { log("Анимация подбора: профиль игрока не найден — запустите игру один раз", "skip"); }
    }

    // Steam updates
    if o.on("steamblock") {
        if let Some(p) = acf_path(game) {
            let s = fs::read_to_string(&p).unwrap_or_default();
            let mut out = String::new();
            for l in s.split_inclusive('\n') {
                if l.contains("\"AutoUpdateBehavior\"") {
                    let ind: String = l.chars().take_while(|c| c.is_whitespace()).collect();
                    out += &format!("{ind}\"AutoUpdateBehavior\"\t\t\"1\"{}", if l.ends_with("\r\n") { "\r\n" } else if l.ends_with('\n') { "\n" } else { "" });
                } else { out += l; }
            }
            write(&mut b, &p, out.as_bytes())?; // no read-only flag: it makes the game hang in the main menu
            log("Steam: автообновление игры отключено", "ok");
        } else { log("Steam: не версия Steam — пропущено", "skip"); }
    }
    Ok(())
}

pub fn title(f: &str) -> &'static str {
    match f {
        "sprint" => "Спринт на отдельной клавише", "sneak" => "Красться на отдельной клавише", "block" => "Мгновенный блок",
        "quickload" => "Кнопка быстрой загрузки", "saves" => "Удобный список сохранений", "noblur" => "Без размытия",
        "borderless" => "Окно без рамки", "instant" => "Мгновенное открытие окон", "wpnfast" => "Ускоренное извлечение оружия", _ => "функция",
    }
}
