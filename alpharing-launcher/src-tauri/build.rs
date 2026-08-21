use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    if let Err(e) = copy_steam_redistributable() {
        println!("cargo:warning=failed to copy steam_api redistributable: {e}");
    }

    tauri_build::build()
}

/// The `steamworks` crate only links `steam_api(64)` into the build script's
/// own OUT_DIR; it never ends up next to the compiled exe on its own.
///
/// On Windows we embed the DLL bytes into the binary (see `src/lib.rs`,
/// `include_bytes!(concat!(env!("OUT_DIR"), ...))`) and delay-load the
/// import so the process can start without the DLL present on disk; at
/// startup the app writes its embedded copy out next to the exe before the
/// first real Steamworks call, so a single portable exe works without an
/// MSI/installer. On other platforms we just drop the shared library next
/// to the built binary, matching how `cargo run`/`cargo build` already work
/// there.
fn copy_steam_redistributable() -> std::io::Result<()> {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    let (redist_dir, lib_name) = match target_os.as_str() {
        "windows" if target_arch == "x86_64" => ("win64", "steam_api64.dll"),
        "windows" => ("win32", "steam_api.dll"),
        "linux" => (
            match target_arch.as_str() {
                "x86_64" => "linux64",
                "x86" => "linux32",
                "aarch64" => "linuxarm64",
                _ => return Ok(()),
            },
            "libsteam_api.so",
        ),
        "macos" => ("osx", "libsteam_api.dylib"),
        _ => return Ok(()),
    };

    let sdk_loc = match env::var("STEAM_SDK_LOCATION") {
        Ok(loc) => PathBuf::from(loc),
        Err(_) => match find_bundled_sdk()? {
            Some(p) => p,
            None => return Ok(()),
        },
    };

    let src = sdk_loc
        .join("redistributable_bin")
        .join(redist_dir)
        .join(lib_name);

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    if target_os == "windows" {
        // Always leave *some* file at this path: `include_bytes!` in
        // lib.rs needs it to exist at compile time even in the (unlikely)
        // case the SDK couldn't be located, so a missing SDK becomes a
        // runtime "couldn't connect to Steam" error instead of a hard
        // compile failure.
        let embed_dest = out_dir.join(lib_name);
        if src.exists() {
            fs::copy(&src, &embed_dest)?;
            println!("cargo:rerun-if-changed={}", src.display());
        } else if !embed_dest.exists() {
            fs::write(&embed_dest, [])?;
        }

        if target_env == "msvc" {
            println!("cargo:rustc-link-arg-bins=/DELAYLOAD:{lib_name}");
            println!("cargo:rustc-link-arg-bins=delayimp.lib");
        } else {
            println!(
                "cargo:warning=steam_api delay-load is only wired up for the MSVC toolchain; \
                 {lib_name} must be placed next to the exe manually on this target"
            );
        }

        return Ok(());
    }

    if !src.exists() {
        return Ok(());
    }

    // OUT_DIR looks like target/<profile>/build/<pkg>-<hash>/out; the profile
    // directory (where the final binary lives) is three levels up from there.
    let profile_dir = out_dir
        .ancestors()
        .nth(3)
        .expect("unexpected OUT_DIR layout");

    fs::copy(&src, profile_dir.join(lib_name))?;
    println!("cargo:rerun-if-changed={}", src.display());

    Ok(())
}

/// `steamworks-sys` bundles the redistributable SDK binaries inside its own
/// crate source under `lib/steam`. Locate it in the local cargo registry
/// cache so we don't have to depend on build-script execution order.
fn find_bundled_sdk() -> std::io::Result<Option<PathBuf>> {
    let cargo_home = env::var("CARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".cargo"));

    let registry_src = cargo_home.join("registry").join("src");
    if !registry_src.exists() {
        return Ok(None);
    }

    for index_entry in fs::read_dir(&registry_src)? {
        let index_entry = index_entry?;
        if !index_entry.file_type()?.is_dir() {
            continue;
        }
        for crate_entry in fs::read_dir(index_entry.path())? {
            let crate_entry = crate_entry?;
            if crate_entry
                .file_name()
                .to_string_lossy()
                .starts_with("steamworks-sys-")
            {
                let sdk = crate_entry.path().join("lib").join("steam");
                if sdk.exists() {
                    return Ok(Some(sdk));
                }
            }
        }
    }

    Ok(None)
}

fn home_dir() -> PathBuf {
    env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default()
}
