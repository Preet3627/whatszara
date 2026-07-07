fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let exe_name = match target_os.as_str() {
        "windows" => "bin/whatsapp-bridge-windows.exe",
        "linux" => "bin/whatsapp-bridge-linux",
        _ => "bin/whatsapp-bridge-darwin",
    };

    let tauri_config = format!(r#"{{"bundle":{{"resources":["{}"]}}}}"#, exe_name);
    std::env::set_var("TAURI_CONFIG", &tauri_config);

    tauri_build::build()
}
