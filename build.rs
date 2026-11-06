// Paper Pilot - bouwscript: versienummer, pictogram, manifest en versie-informatie in de .exe.
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=versie.txt");
    println!("cargo:rerun-if-changed=res/paperpilot.ico");
    let versie = fs::read_to_string("versie.txt").expect("versie.txt").trim().to_string();
    let delen: Vec<u16> = versie.split('.').map(|d| d.parse().expect("versie")).collect();
    assert_eq!(delen.len(), 4, "versie.txt moet vier delen hebben, zoals 0.0.0.1");
    println!("cargo:rustc-env=PP_VERSIE={versie}");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let uit = PathBuf::from(env::var("OUT_DIR").unwrap());
    let schuin = |p: &PathBuf| p.to_string_lossy().replace('\\', "/");
    let manifest = uit.join("app.manifest");
    fs::write(&manifest, format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="JaapWortel.PaperPilot" version="{versie}" processorArchitecture="amd64"/>
  <dependency><dependentAssembly>
    <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
      processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
  </dependentAssembly></dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges>
    <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
  </requestedPrivileges></security></trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1"><application>
    <supportedOS Id="{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}"/>
  </application></compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3"><windowsSettings>
    <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
    <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
  </windowsSettings></application>
</assembly>
"#)).unwrap();
    let ico = schuin(&PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("res/paperpilot.ico"));
    let k = delen.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(",");
    let rc = uit.join("app.rc");
    fs::write(&rc, format!(r#"1 ICON "{ico}"
1 24 "{man}"
1 VERSIONINFO
FILEVERSION {k}
PRODUCTVERSION {k}
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "Jaap Wortel"
      VALUE "FileDescription", "Paper Pilot"
      VALUE "FileVersion", "{versie}"
      VALUE "InternalName", "PaperPilot"
      VALUE "LegalCopyright", "Jaap Wortel, GPL-3.0"
      VALUE "OriginalFilename", "PaperPilot.exe"
      VALUE "ProductName", "Paper Pilot"
      VALUE "ProductVersion", "{versie}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#, man = schuin(&manifest))).unwrap();
    embed_resource::compile(&rc, embed_resource::NONE).manifest_optional().unwrap();
}
