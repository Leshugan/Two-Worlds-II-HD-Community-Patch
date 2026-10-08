// WebView2Loader.dll is embedded and loaded at runtime, so the installer stays a single exe.
// The import slots that webview2-com-sys calls through (__imp_*) are defined here and filled after LoadLibrary.
#![allow(non_upper_case_globals)]
use std::ffi::c_void;

#[no_mangle] pub static mut __imp_CompareBrowserVersions: usize = 0;
#[no_mangle] pub static mut __imp_CreateCoreWebView2Environment: usize = 0;
#[no_mangle] pub static mut __imp_CreateCoreWebView2EnvironmentWithOptions: usize = 0;
#[no_mangle] pub static mut __imp_GetAvailableCoreWebView2BrowserVersionString: usize = 0;
#[no_mangle] pub static mut __imp_GetAvailableCoreWebView2BrowserVersionStringWithOptions: usize = 0;

// direct-call thunks (jump through the slots above)
std::arch::global_asm!(
    ".globl CompareBrowserVersions", "CompareBrowserVersions: jmp qword ptr [rip + __imp_CompareBrowserVersions]",
    ".globl CreateCoreWebView2Environment", "CreateCoreWebView2Environment: jmp qword ptr [rip + __imp_CreateCoreWebView2Environment]",
    ".globl CreateCoreWebView2EnvironmentWithOptions", "CreateCoreWebView2EnvironmentWithOptions: jmp qword ptr [rip + __imp_CreateCoreWebView2EnvironmentWithOptions]",
    ".globl GetAvailableCoreWebView2BrowserVersionString", "GetAvailableCoreWebView2BrowserVersionString: jmp qword ptr [rip + __imp_GetAvailableCoreWebView2BrowserVersionString]",
    ".globl GetAvailableCoreWebView2BrowserVersionStringWithOptions", "GetAvailableCoreWebView2BrowserVersionStringWithOptions: jmp qword ptr [rip + __imp_GetAvailableCoreWebView2BrowserVersionStringWithOptions]",
);

static DLL: &[u8] = include_bytes!("../wv2/WebView2Loader.dll");

extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(m: *mut c_void, name: *const u8) -> usize;
}

pub fn init() -> Result<(), String> {
    let dir = std::env::temp_dir().join("TW2CommunityPatch");
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("WebView2Loader.dll");
    if std::fs::read(&p).map(|d| d != DLL).unwrap_or(true) {
        std::fs::write(&p, DLL).map_err(|e| format!("{e}"))?;
    }
    let w: Vec<u16> = p.as_os_str().to_string_lossy().encode_utf16().chain(Some(0)).collect();
    unsafe {
        let m = LoadLibraryW(w.as_ptr());
        if m.is_null() { return Err("WebView2Loader".into()); }
        let set = |slot: *mut usize, n: &[u8]| { *slot = GetProcAddress(m, n.as_ptr()); };
        set(std::ptr::addr_of_mut!(__imp_CompareBrowserVersions), b"CompareBrowserVersions\0");
        set(std::ptr::addr_of_mut!(__imp_CreateCoreWebView2Environment), b"CreateCoreWebView2Environment\0");
        set(std::ptr::addr_of_mut!(__imp_CreateCoreWebView2EnvironmentWithOptions), b"CreateCoreWebView2EnvironmentWithOptions\0");
        set(std::ptr::addr_of_mut!(__imp_GetAvailableCoreWebView2BrowserVersionString), b"GetAvailableCoreWebView2BrowserVersionString\0");
        set(std::ptr::addr_of_mut!(__imp_GetAvailableCoreWebView2BrowserVersionStringWithOptions), b"GetAvailableCoreWebView2BrowserVersionStringWithOptions\0");
    }
    Ok(())
}
