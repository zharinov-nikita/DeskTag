fn main() {
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rerun-if-changed=assets/desktag.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/desktag.ico");
        if let Err(e) = res.compile() {
            eprintln!("warning: failed to embed icon resource: {e}");
        }
    }
}
