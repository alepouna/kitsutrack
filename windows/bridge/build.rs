fn main() {
    #[cfg(windows)]
    tauri_build::try_build(
        tauri_build::Attributes::new().windows_attributes(
            tauri_build::WindowsAttributes::new()
                .window_icon_path("../../shared/assets/kitsutrack/icon.ico"),
        ),
    )
    .expect("build Tauri resources");

    #[cfg(not(windows))]
    tauri_build::build();
}
