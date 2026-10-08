#![windows_subsystem = "windows"]
mod blob;
mod install;
mod patches;
mod pe;
mod voices;
mod wv2;

#[link(name = "user32")]
extern "system" { fn MessageBoxW(h: isize, t: *const u16, c: *const u16, f: u32) -> i32; }
fn fatal(t: &str) -> ! {
    let w = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    unsafe { MessageBoxW(0, w(t).as_ptr(), w("Two Worlds II HD Community Patch").as_ptr(), 0x10); }
    std::process::exit(1)
}

use serde_json::{json, Value};
use std::path::PathBuf;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Theme, WindowBuilder};
use wry::WebViewBuilder;

pub const DXVK: [(&str, &[u8]); 4] = [
    ("d3d9.dll", include_bytes!("../dxvk/d3d9.dll")),
    ("d3d10core.dll", include_bytes!("../dxvk/d3d10core.dll")),
    ("d3d11.dll", include_bytes!("../dxvk/d3d11.dll")),
    ("dxgi.dll", include_bytes!("../dxvk/dxgi.dll")),
];
const PROJECT_URL: &str = "https://github.com/Leshugan";

enum Ev { Js(String) }

fn js(p: &EventLoopProxy<Ev>, s: String) { let _ = p.send_event(Ev::Js(s)); }
fn q<T: serde::Serialize + ?Sized>(v: &T) -> String { serde_json::to_string(v).unwrap_or_else(|_| "null".into()) }

fn handle(msg: &str, p: &EventLoopProxy<Ev>) {
    let v: Value = match serde_json::from_str(msg) { Ok(v) => v, Err(_) => return };
    let path = PathBuf::from(v["path"].as_str().unwrap_or_default());
    let p = p.clone();
    match v["cmd"].as_str().unwrap_or_default() {
        "init" => { std::thread::spawn(move || {
            let list: Vec<Value> = install::scan().into_iter().map(|(p, l)| json!({"path": p, "label": l})).collect();
            let (w, h, fov) = install::screen_fov();
            js(&p, format!("onInit({})", q(&json!({"installs": list, "fov": fov, "screen": format!("{w}×{h}")}))));
        }); }
        "browse" => { std::thread::spawn(move || {
            if let Some(f) = rfd::FileDialog::new().set_title("Папка игры (где лежит TwoWorlds2.exe / TwoWorlds2_DX10.exe)").pick_folder() {
                js(&p, format!("onPath({})", q(&f.to_string_lossy())));
            }
        }); }
        "check" => { std::thread::spawn(move || { let r = install::check(&path); js(&p, format!("onCheck({})", q(&r))); }); }
        "install" | "uninstall" => {
            let inst = v["cmd"] == "install";
            let opts = install::Opts { v: v["opts"].clone() };
            std::thread::spawn(move || {
                let log = |t: &str, c: &str| js(&p, format!("onLog({},{})", q(t), q(c)));
                let r = if inst { install::install(&path, &opts, &log) } else { install::uninstall(&path, &log) };
                match r {
                    Ok(_) if inst => log("Готово. Патч установлен — можно запускать игру.", "ok"),
                    Ok(_) => log("Готово. Патч удалён, игра возвращена к состоянию до установки.", "ok"),
                    Err(e) => log(&e, "bad"),
                }
                js(&p, "onDone()".into());
            });
        }
        "link" => { let _ = std::process::Command::new("explorer").arg(PROJECT_URL).spawn(); }
        _ => {}
    }
}

// Developer self-test: TW2_Community_Patch.exe --patch <in.exe> <out.exe>
fn selftest(a: &[String]) {
    let mut d = std::fs::read(&a[2]).unwrap();
    let o = patches::BinOpts { input: true, saves: true, quickload: true, noblur: true, borderless: true, laa: true, far_draw: true, far_grass: true, wpnfast: true, instant: true,
        sprint_mode: 1, sneak_mode: 2, sprint_vk: 0x10, sneak_vk: 0x43, ql_vk: 0x75 };
    let r = patches::apply(&mut d, &o);
    std::fs::write(&a[3], &d).unwrap();
    std::fs::write(format!("{}.log", a[3]), format!("na={:?}\n{}", r.na, r.log.join("\n"))).unwrap();
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() == 4 && a[1] == "--patch" { selftest(&a); return; }
    if a.len() == 4 && (a[1] == "--install" || a[1] == "--uninstall") {
        // developer test: --install <game dir> <log file>  (all default options)
        let lines = std::cell::RefCell::new(String::new());
        let log = |t: &str, c: &str| { lines.borrow_mut().push_str(&format!("[{c}] {t}\n")); };
        let o = install::Opts { v: json!({"sprint":true,"sneak":true,"quickload":true,"block":true,"camera":true,"dxvk":true,"draw":true,
            "horizon":true,"lodtrees":true,"noblur":true,"saves":true,"noloot":false,"instant":true,"wpnfast":true,"shadows":true,"grass":true,"fov":true,"intro":true,"borderless":true,"laa":true,"voices":true,"steamblock":true,
            "sprint_key":16,"sneak_key":67,"quickload_key":117,"sprint_mode":1,"sneak_mode":2,"draw_level":10,"grass_level":3}) };
        let g = PathBuf::from(&a[2]);
        let r = if a[1] == "--install" { install::install(&g, &o, &log) } else { install::uninstall(&g, &log) };
        let mut s = lines.into_inner();
        s += &format!("result: {:?}\ncheck: {}\n", r, install::check(&g));
        let _ = std::fs::write(&a[3], s);
        return;
    }
    let event_loop = EventLoopBuilder::<Ev>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let (mut w, mut h) = (1200.0, 950.0);
    if let Some(m) = event_loop.primary_monitor() {
        let s = m.size().to_logical::<f64>(m.scale_factor());
        w = f64::min(w, s.width * 0.95);
        h = f64::min(h, s.height * 0.82);
    }
    let window = WindowBuilder::new()
        .with_title("Two Worlds II HD Community Patch")
        .with_inner_size(LogicalSize::new(w, h))
        .with_min_inner_size(LogicalSize::new(900.0, 600.0))
        .with_theme(Some(Theme::Dark))
        .with_window_icon(<tao::window::Icon as tao::platform::windows::IconExtWindows>::from_resource(1, None).ok())
        .build(&event_loop)
        .expect("window");
    const NO_WV2: &str = "Не удалось открыть окно установщика.\n\nУстановите компонент Microsoft Edge WebView2 Runtime (бесплатно, с сайта Microsoft) и запустите установщик снова.";
    if wv2::init().is_err() { fatal(NO_WV2); }
    let data = std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or(std::env::temp_dir()).join("TW2CommunityPatch").join("WebView2");
    let mut ctx = wry::WebContext::new(Some(data));
    let p2 = proxy.clone();
    let webview = WebViewBuilder::with_web_context(&mut ctx)
        .with_html(include_str!("../ui/index.html"))
        .with_background_color((22, 17, 12, 255))
        .with_devtools(false)
        .with_ipc_handler(move |req: wry::http::Request<String>| handle(req.body(), &p2))
        .build(&window)
        .unwrap_or_else(|_| fatal(NO_WV2));
    event_loop.run(move |event, _, cf| {
        *cf = ControlFlow::Wait;
        match event {
            Event::UserEvent(Ev::Js(s)) => { let _ = webview.evaluate_script(&s); }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *cf = ControlFlow::Exit,
            _ => {}
        }
    });
}
