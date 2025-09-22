#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use stratosort::run;

fn setup_panic_handler() {
    use std::panic;
    use std::fs::File;
    use std::io::Write;

    panic::set_hook(Box::new(|panic_info| {
        let message = format!(
            "PANIC: {}\nLocation: {:?}\n",
            panic_info,
            panic_info.location()
        );

        eprintln!("{}", message);

        // Write crash dump
        if let Ok(mut file) = File::create("crash.log") {
            let _ = writeln!(file, "{}", message);
            let _ = writeln!(file, "Timestamp: {}", chrono::Utc::now());
        }
    }));
}

fn main() {
    // Set up panic handler before anything else
    setup_panic_handler();

    if let Err(e) = run() {
        eprintln!("Application error: {}", e);
        std::process::exit(1);
    }
}
