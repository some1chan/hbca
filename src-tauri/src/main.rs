// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use std::ffi::CString;
use std::path::Path;
use std::{thread::sleep, time::Duration};
use tauri::Manager;
use winapi::um::winuser::{FindWindowA, SetForegroundWindow};

#[tauri::command]
fn focus_window(window_name: String) -> String {
    let window_title = CString::new(window_name).expect("Failed to create CString");

    // Returns the handle of the window
    let handle = unsafe { FindWindowA(std::ptr::null_mut(), window_title.as_ptr()) };
    println!("Using handle {:?}", handle);
    unsafe { SetForegroundWindow(handle) };

    format!("{:?}", handle)
}

#[tauri::command]
fn press_key(key: char, hold_time: u64) -> Result<(), String> {
    // let static_char: &'static str = Box::leak(key.into_boxed_str());
    // inputbot::KeySequence(static_char).send();

    let keybd_key = inputbot::get_keybd_key(key);
    if let Some(keybd_key) = keybd_key {
        keybd_key.press();
        sleep(Duration::from_millis(hold_time));
        keybd_key.release();
        Ok(())
    } else {
        Err(format!("Failed to find key for character: {}", key))
    }
    // Free the previously leaked memory
    // when it's no longer needed
    // unsafe {
    //     drop(Box::from_raw(static_char as *const str as *mut str));
    // }
}

fn get_path_to_game_settings() -> String {
    let appdata = std::env::var("APPDATA").unwrap();

    let unbeatable_path = format!(
        "{}\\..\\LocalLow\\D-CELL GAMES\\UNBEATABLE [white label]",
        appdata
    );

    let settings_path = format!("{}\\SYSTEM\\system-options.json", unbeatable_path);

    return settings_path;
}

#[tauri::command]
fn get_offset_from_game_settings() -> Result<f64, String> {
    let settings_path = get_path_to_game_settings();

    // check if the file exists
    let exists = std::path::Path::new(&settings_path).exists();

    if !exists {
        return Err(format!("Could not find settings file at {}", settings_path));
    }

    let file = match std::fs::read_to_string(settings_path) {
        Ok(file) => file,
        Err(e) => return Err(format!("Failed to read settings file: {}", e)),
    };

    let json: serde_json::Value = match serde_json::from_str(&file) {
        Ok(json) => json,
        Err(e) => return Err(format!("Failed to parse settings file: {}", e)),
    };

    let offset = match json["rhythmTrackerPositionOffset"].as_f64() {
        Some(offset) => offset,
        None => return Err(format!("Failed to get offset from settings file")),
    };

    Ok(offset as f64)
}

fn main() {
    // inputbot::init_device();
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle();

            // spawn a new thread to run the watcher
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(3000));

                let settings_path = get_path_to_game_settings();

                // check if the file exists
                let exists = std::path::Path::new(&settings_path).exists();

                if !exists {
                    println!(
                        "Could not find settings file at {}, not running the watcher",
                        settings_path
                    );
                    return;
                }

                // Select recommended watcher for debouncer.
                // Using a callback here, could also be a channel.
                let mut debouncer = new_debouncer(
                    Duration::from_millis(10),
                    None,
                    move |res: DebounceEventResult| match res {
                        Ok(_events) => {
                            let offset = get_offset_from_game_settings();

                            match offset {
                                Ok(offset) => {
                                    handle
                                        .emit_all("config_changed", Some(offset))
                                        .expect("failed to emit");
                                }
                                Err(e) => {
                                    handle
                                        .emit_all("config_changed", Some(e))
                                        .expect("failed to emit");
                                }
                            }
                        }
                        Err(e) => println!("watch error: {:?}", e),
                    },
                )
                .unwrap();

                // Add a path to be watched. All files and directories at that path and
                // below will be monitored for changes.
                debouncer
                    .watcher()
                    .watch(Path::new(&settings_path), RecursiveMode::Recursive)
                    .unwrap();

                // Do I need this?
                loop {
                    interval.tick().await;
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            focus_window,
            press_key,
            get_offset_from_game_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
