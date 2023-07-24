// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

use std::ffi::CString;
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
fn press_key(key: char) -> Result<(), String> {
    // let static_char: &'static str = Box::leak(key.into_boxed_str());
    // inputbot::KeySequence(static_char).send();

    let keybd_key = inputbot::get_keybd_key(key);
    if let Some(keybd_key) = keybd_key {
        keybd_key.press();
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

fn main() {
    // inputbot::init_device();
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet, focus_window, press_key])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
