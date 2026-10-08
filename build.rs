// Windows resources (icon, version info, manifest) compiled with windres and linked into the exe.
use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    let manifest = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges>
<requestedExecutionLevel level="highestAvailable" uiAccess="false"/></requestedPrivileges></security></trustInfo>
<application xmlns="urn:schemas-microsoft-com:asm.v3"><windowsSettings>
<dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
<dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
</windowsSettings></application>
<dependency><dependentAssembly><assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/></dependentAssembly></dependency>
</assembly>"#;
    fs::write(out.join("app.manifest"), manifest).unwrap();
    let esc = |p: PathBuf| p.to_string_lossy().replace('\\', "/");
    let rc = format!(r#"#pragma code_page(65001)
1 ICON "{icon}"
1 24 "{man}"
1 VERSIONINFO
FILEVERSION 1,0,0,0
PRODUCTVERSION 1,0,0,0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "041904B0"
    BEGIN
      VALUE "CompanyName", "Leshugan"
      VALUE "FileDescription", "Two Worlds II HD Community Patch"
      VALUE "ProductName", "Two Worlds II HD Community Patch"
      VALUE "FileVersion", "1.0"
      VALUE "ProductVersion", "1.0"
      VALUE "OriginalFilename", "TW2_Community_Patch.exe"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x419, 1200
  END
END
"#, icon = esc(dir.join("icon.ico")), man = esc(out.join("app.manifest")));
    fs::write(out.join("res.rc"), rc).unwrap();
    let obj = out.join("res.o");
    let ok = Command::new("windres").arg(out.join("res.rc")).args(["-O", "coff", "-o"]).arg(&obj).status().map(|s| s.success()).unwrap_or(false);
    if ok { println!("cargo:rustc-link-arg-bins={}", obj.display()); } else { println!("cargo:warning=windres failed: no icon/version info"); }
    // replace mingw's default-manifest.o (asInvoker) with an empty object so our manifest is the only one
    let dm = env::temp_dir().join("tw2cp_dm"); // gcc splits -B paths at spaces, so keep it out of the project folder
    let _ = fs::create_dir_all(&dm);
    fs::write(dm.join("empty.rc"), "STRINGTABLE\nBEGIN\n  999, \"TW2CP\"\nEND\n").unwrap();
    let ok2 = Command::new("windres").arg(dm.join("empty.rc")).args(["-O", "coff", "-o"]).arg(dm.join("default-manifest.o")).status().map(|s| s.success()).unwrap_or(false);
    if ok2 { println!("cargo:rustc-link-arg-bins=-B{}/", esc(dm)); }
}
