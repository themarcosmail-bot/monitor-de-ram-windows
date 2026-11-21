use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let source = output.join("version.rc");
    let version = env!("CARGO_PKG_VERSION");
    let version_parts = format!("{},0", version.replace('.', ","));
    let resource = format!(
        r#"LANGUAGE 0x16, 0x01
1 VERSIONINFO
FILEVERSION {version_parts}
PRODUCTVERSION {version_parts}
FILEFLAGSMASK 0x3f
FILEFLAGS 0
FILEOS 0x00040004
FILETYPE 1
FILESUBTYPE 0
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "041604E4"
    BEGIN
      VALUE "CompanyName", "themarcosmail-bot\0"
      VALUE "FileDescription", "Monitor de RAM\0"
      VALUE "FileVersion", "{version}\0"
      VALUE "InternalName", "monitor-de-ram\0"
      VALUE "OriginalFilename", "Monitor de RAM.exe\0"
      VALUE "ProductName", "Monitor de RAM\0"
      VALUE "ProductVersion", "{version}\0"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x0416, 1252
  END
END
"#
    );
    fs::write(&source, resource).expect("write the Windows version resource");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=RC");
    println!("cargo:rerun-if-env-changed=WINDRES");

    let (mut compiler, compiled) = match env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("msvc") => {
            let compiled = output.join("version.res");
            let mut compiler = Command::new(msvc_rc());
            compiler
                .arg("/nologo")
                .arg("/fo")
                .arg(&compiled)
                .arg(&source);
            (compiler, compiled)
        }
        Ok("gnu") => {
            let compiled = output.join("version.o");
            let mut compiler =
                Command::new(env::var_os("WINDRES").unwrap_or_else(|| "windres.exe".into()));
            compiler
                .arg("-O")
                .arg("coff")
                .arg("-i")
                .arg(&source)
                .arg("-o")
                .arg(&compiled);
            (compiler, compiled)
        }
        _ => panic!("build the app with a Windows MSVC or GNU toolchain"),
    };
    let status = compiler
        .status()
        .expect("run the Windows resource compiler");
    assert!(
        status.success(),
        "failed to compile Windows version metadata"
    );
    println!(
        "cargo:rustc-link-arg-bin=monitor-de-ram={}",
        compiled.display()
    );
}

fn msvc_rc() -> PathBuf {
    if let Some(path) = env::var_os("RC") {
        return path.into();
    }
    let sdk = env::var_os("ProgramFiles(x86)")
        .map(PathBuf::from)
        .map(|path| path.join("Windows Kits").join("10").join("bin"));
    let newest = sdk
        .and_then(|root| fs::read_dir(root).ok())
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("x64").join("rc.exe"))
        .filter(|path| path.is_file())
        .max();
    newest.unwrap_or_else(|| PathBuf::from("rc.exe"))
}
