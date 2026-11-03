//! Open a web page in the user's browser (Nextcloud's login page, VRT-39), with the OS's own
//! opener: no plugin needed.

pub fn open(url: &str) {
    // Tests drive the app without a desktop browser (they approve the login themselves).
    if std::env::var_os("VERITAS_NO_BROWSER").is_some() {
        eprintln!("open in the browser: {url}");
        return;
    }
    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    if let Err(e) = result {
        eprintln!("opening {url} failed: {e}");
    }
}
