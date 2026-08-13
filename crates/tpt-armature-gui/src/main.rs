fn main() {
    #[cfg(feature = "app")]
    {
        if let Err(e) = tpt_armature_gui::run() {
            eprintln!("gui error: {e}");
            std::process::exit(1);
        }
    }
    #[cfg(not(feature = "app"))]
    {
        eprintln!("The tpt-armature-gui binary requires the `app` feature.");
        eprintln!("Build with: cargo run -p tpt-armature-gui --features app -- <binary>");
        std::process::exit(1);
    }
}
