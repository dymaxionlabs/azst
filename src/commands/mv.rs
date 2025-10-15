use anyhow::{anyhow, Result};
use colored::*;

use crate::commands::{cp, rm};
use crate::utils::is_azure_uri;

pub async fn execute(source: &str, destination: &str, recursive: bool, force: bool) -> Result<()> {
    let source_is_azure = is_azure_uri(source);
    let dest_is_azure = is_azure_uri(destination);

    // Validate that at least one side is Azure
    if !source_is_azure && !dest_is_azure {
        return Err(anyhow!(
            "Move operation requires at least one Azure path. Use 'mv' shell command for local moves."
        ));
    }

    println!(
        "{} {} {} to {}",
        "⇄".green(),
        "Moving".bold(),
        source.cyan(),
        destination.cyan()
    );

    // Step 1: Copy the source to destination
    println!("{} Step 1: Copying files...", "→".dimmed());
    cp::execute(source, destination, recursive).await?;

    // Step 2: Remove the source
    println!("{} Step 2: Removing source files...", "×".dimmed());
    rm::execute(source, recursive, force).await?;

    println!("{} Move operation completed successfully", "✓".green());
    Ok(())
}
