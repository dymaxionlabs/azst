use anyhow::Result;
use clap::Parser;
use std::io::{self, ErrorKind};

mod azcopy_output;
mod azure;
mod cli;
mod commands;
mod output;
mod utils;

use cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    // Set up a custom panic hook to handle broken pipe errors gracefully
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Check if this is a broken pipe error
        if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            if s.contains("Broken pipe") {
                // Silently exit on broken pipe (e.g., when piping to head)
                std::process::exit(0);
            }
        } else if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            if s.contains("Broken pipe") {
                std::process::exit(0);
            }
        }
        // For other panics, use the default handler
        default_panic(panic_info);
    }));

    let cli = Cli::parse();

    match cli.run().await {
        Ok(_) => {}
        Err(e) => {
            // Check if the error is a broken pipe error
            if let Some(io_err) = e.downcast_ref::<io::Error>() {
                if io_err.kind() == ErrorKind::BrokenPipe {
                    // Silently exit on broken pipe
                    std::process::exit(0);
                }
            }
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
