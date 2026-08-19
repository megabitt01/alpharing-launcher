use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

const GAME_SUBPATH: &str = "steamapps/common/Halo The Master Chief Collection";
const MOD_DLL_SUBPATH: &str = "MCC/Binaries/Win64";
const MOD_DLL_NAME: &str = "WTSAPI32.dll";
const EXTRA_MOD_FILES: &[&str] = &["alpha_ring_menu.bin", "alpha_ring_menu.cfg"];
const RELEASES_API_URL: &str = "https://api.github.com/repos/megabitt01/AlphaRing/releases/latest";
const ANTICHEAT_FLAG: &str = "-eac";

struct AppPaths {
    game_path: PathBuf,
    backup_path: PathBuf,
}

static APP_PATHS: OnceLock<Mutex<Option<AppPaths>>> = OnceLock::new();

fn app_paths_lock() -> &'static Mutex<Option<AppPaths>> {
    APP_PATHS.get_or_init(|| Mutex::new(None))
}

fn set_app_paths(game_path: PathBuf) {
    let backup_path = game_path.join("alpha_ring");
    *app_paths_lock().lock().unwrap() = Some(AppPaths {
        game_path,
        backup_path,
    });
}

fn get_app_paths() -> Result<(PathBuf, PathBuf), String> {
    app_paths_lock()
        .lock()
        .unwrap()
        .as_ref()
        .map(|paths| (paths.game_path.clone(), paths.backup_path.clone()))
        .ok_or_else(|| "Game path has not been detected yet; run checkOs first".to_string())
}

fn emit_log(app: &AppHandle, message: impl Into<String>) {
    let message = message.into();
    println!("{message}");
    let _ = app.emit("log", message);
}

#[cfg(target_os = "windows")]
fn steam_root_candidates() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("C:/Program Files (x86)/Steam"),
        PathBuf::from("C:/Program Files/Steam"),
    ];

    for letter in b'D'..=b'Z' {
        for name in ["Steam", "SteamLibrary"] {
            roots.push(PathBuf::from(format!("{}:/{}", letter as char, name)));
        }
    }

    roots
}

#[cfg(target_os = "linux")]
fn steam_root_candidates() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        // Covers the native Debian/Flatpak/Steam Deck install layouts; whichever
        // one is real is the one that actually has a steamapps directory in it.
        roots.push(home.join(".local/share/Steam"));
        roots.push(home.join(".steam/steam"));
        roots.push(home.join(".steam/root"));
        roots.push(home.join(".var/app/com.valvesoftware.Steam/data/Steam"));
    }

    roots
}

// Steam records every library folder the user has added (including ones on
// other drives) in this VDF file next to the main Steam install's steamapps.
fn parse_library_folders(vdf_path: &Path) -> Vec<PathBuf> {
    let Ok(contents) = fs::read_to_string(vdf_path) else {
        return Vec::new();
    };

    contents
        .lines()
        .filter(|line| line.trim_start().starts_with("\"path\""))
        .filter_map(|line| line.split('"').nth(3))
        .map(PathBuf::from)
        .collect()
}

fn find_game_path() -> Option<PathBuf> {
    for steam_root in steam_root_candidates() {
        let steamapps = steam_root.join("steamapps");
        if !steamapps.is_dir() {
            continue;
        }

        let mut libraries = vec![steam_root];
        libraries.extend(parse_library_folders(&steamapps.join("libraryfolders.vdf")));

        for library in libraries {
            let candidate = library.join(GAME_SUBPATH);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }

    None
}

fn check_os(app: &AppHandle) -> Result<(), String> {
    if cfg!(target_os = "windows") {
        emit_log(app, "Running Windows version");
    } else if cfg!(target_os = "linux") {
        emit_log(app, "Running Linux version");
    }

    emit_log(app, "Checking MCC installation...");

    let game_path = find_game_path().ok_or_else(|| {
        "Couldn't find MCC".to_string()
    })?;

    emit_log(app, format!("Found game installation at {}", game_path.display()));
    set_app_paths(game_path);
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn sha256_of_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    Ok(sha256_hex(&bytes))
}

fn move_extra_mod_files(src_dir: &Path, dest_dir: &Path) -> Result<(), String> {
    for file_name in EXTRA_MOD_FILES {
        let src = src_dir.join(file_name);
        if !src.exists() {
            continue;
        }

        fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
        fs::rename(&src, dest_dir.join(file_name)).map_err(|e| e.to_string())?;
    }

    Ok(())
}

async fn fetch_latest_release(client: &reqwest::Client) -> Result<serde_json::Value, String> {
    client
        .get(RELEASES_API_URL)
        .header("User-Agent", "AlphaRing-Launcher")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

async fn fetch_latest_release_asset(client: &reqwest::Client) -> Result<Vec<u8>, String> {
    let release = fetch_latest_release(client).await?;

    let assets = release["assets"]
        .as_array()
        .ok_or("Release contains no assets")?;

    let asset = assets
        .iter()
        .find(|asset| asset["name"].as_str() == Some(MOD_DLL_NAME))
        .ok_or("WTSAPI32.dll was not found in the latest release")?;

    let download_url = asset["browser_download_url"]
        .as_str()
        .ok_or("Release asset has no download URL")?;

    let bytes = client
        .get(download_url)
        .header("User-Agent", "AlphaRing-Launcher")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;

    Ok(bytes.to_vec())
}

async fn install_mod(app: &AppHandle, target_dir: &Path) -> Result<String, String> {
    if !target_dir.exists() {
        return Err(format!(
            "Directory does not exist: {}",
            target_dir.display()
        ));
    }

    if !target_dir.is_dir() {
        return Err(format!("Path is not a directory: {}", target_dir.display()));
    }

    emit_log(app, "Downloading the latest AlphaRing release...");

    let client = reqwest::Client::new();
    let bytes = fetch_latest_release_asset(&client).await?;
    let destination = target_dir.join(MOD_DLL_NAME);

    fs::write(&destination, bytes).map_err(|e| e.to_string())?;

    emit_log(app, format!("Installed mod to {}", destination.display()));

    Ok(destination.to_string_lossy().to_string())
}

async fn check_mod(app: &AppHandle, vanilla_mode: bool) -> Result<(), String> {
    let (game_path, backup_path) = get_app_paths()?;
    let mod_dir = game_path.join(MOD_DLL_SUBPATH);
    let mod_dll = mod_dir.join(MOD_DLL_NAME);
    let backup_dll = backup_path.join(MOD_DLL_NAME);

    if vanilla_mode {
        if mod_dll.exists() {
            emit_log(app, "Moving mod files aside for vanilla play...");
            fs::create_dir_all(&backup_path).map_err(|e| e.to_string())?;
            fs::rename(&mod_dll, &backup_dll).map_err(|e| e.to_string())?;
        }
        move_extra_mod_files(&mod_dir, &backup_path)?;
        return launch(app, vanilla_mode);
    }

    let mut mod_up_to_date = false;

    if mod_dll.exists() {
        emit_log(app, "Checking installed mod version...");
        let client = reqwest::Client::new();
        let latest_bytes = fetch_latest_release_asset(&client).await?;

        mod_up_to_date = sha256_of_file(&mod_dll)? == sha256_hex(&latest_bytes);
        if mod_up_to_date {
            emit_log(app, "Mod is up to date.");
        }
    }

    if !mod_up_to_date {
        if backup_dll.exists() {
            emit_log(app, "Restoring cached mod files...");
            fs::rename(&backup_dll, &mod_dll).map_err(|e| e.to_string())?;
        } else {
            install_mod(app, &mod_dir).await?;
        }
    }

    move_extra_mod_files(&backup_path, &mod_dir)?;
    launch(app, vanilla_mode)
}

fn launch(app: &AppHandle, vanilla_mode: bool) -> Result<(), String> {
    emit_log(app, "Launching MCC...");

    let mut command = Command::new("steam");
    command.args(["-applaunch", "976730"]);

    if !vanilla_mode {
        if cfg!(target_os = "linux") {
            command.env("WINEDLLOVERRIDES", "WTSAPI32=n,b");
        }
        command.arg(ANTICHEAT_FLAG);
    }

    command.spawn().map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn latest_mod_version() -> Result<String, String> {
    let client = reqwest::Client::new();
    let release = fetch_latest_release(&client).await?;

    release["tag_name"]
        .as_str()
        .map(|tag| tag.to_string())
        .ok_or_else(|| "Release has no tag name".to_string())
}

#[tauri::command]
async fn play(app: AppHandle, vanilla_mode: bool) -> Result<(), String> {
    if let Err(err) = check_os(&app) {
        emit_log(&app, format!("Error: {err}"));
        return Err(err);
    }

    if let Err(err) = check_mod(&app, vanilla_mode).await {
        emit_log(&app, format!("Error: {err}"));
        return Err(err);
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![play, latest_mod_version])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
