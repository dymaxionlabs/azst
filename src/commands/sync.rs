use anyhow::{anyhow, Result};
use colored::*;
use std::io::{self, Write};

use crate::azure::{convert_az_uri_to_url, AzCopyClient};
use crate::utils::{is_azure_uri, parse_azure_uri};

pub async fn execute(
    source: &str,
    destination: &str,
    delete_destination: bool,
    force: bool,
) -> Result<()> {
    let source_is_azure = is_azure_uri(source);
    let dest_is_azure = is_azure_uri(destination);

    // Sync only works with at least one Azure location
    if !source_is_azure && !dest_is_azure {
        return Err(anyhow!(
            "Sync requires at least one Azure location (az://...)"
        ));
    }

    let azcopy = AzCopyClient::new();
    azcopy.check_prerequisites().await?;
    sync_with_azcopy(source, destination, delete_destination, force).await
}

async fn sync_with_azcopy(
    source: &str,
    destination: &str,
    delete_destination: bool,
    force: bool,
) -> Result<()> {
    // Validate Azure URIs
    if is_azure_uri(source) {
        let (_, container, _) = parse_azure_uri(source)?;
        if container.is_empty() {
            return Err(anyhow!(
                "Invalid source URI '{}'. You must specify both storage account and container: az://<account>/<container>/[path]",
                source
            ));
        }
    }

    if is_azure_uri(destination) {
        let (_, container, _) = parse_azure_uri(destination)?;
        if container.is_empty() {
            return Err(anyhow!(
                "Invalid destination URI '{}'. You must specify both storage account and container: az://<account>/<container>/[path]",
                destination
            ));
        }
    }

    // Warn about delete-destination if not forced
    if delete_destination && !force {
        println!(
            "{} {}",
            "⚠".yellow(),
            "Sync with --delete will remove files in destination that don't exist in source!"
                .yellow()
        );
        print!("Continue? (y/N): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!("Aborted");
            return Ok(());
        }
    }

    // Convert az:// URIs to HTTPS URLs for AzCopy
    let source_url = if is_azure_uri(source) {
        convert_az_uri_to_url(source)?
    } else {
        source.to_string()
    };

    let dest_url = if is_azure_uri(destination) {
        convert_az_uri_to_url(destination)?
    } else {
        destination.to_string()
    };

    // Display operation
    let operation_type = match (is_azure_uri(source), is_azure_uri(destination)) {
        (false, true) => "Syncing local to Azure",
        (true, false) => "Syncing Azure to local",
        (true, true) => "Syncing Azure to Azure",
        _ => "Syncing",
    };

    println!(
        "{} {} {} → {} {}",
        "⇄".green(),
        operation_type,
        source.cyan(),
        destination.cyan(),
        if delete_destination {
            "(with delete)".yellow()
        } else {
            "".dimmed()
        }
    );

    // Show the actual AzCopy command for debugging
    let delete_flag = if delete_destination {
        " --delete-destination=true"
    } else {
        ""
    };
    println!(
        "{} {}",
        "⚙".dimmed(),
        format!("azcopy sync '{}' '{}'{}", source_url, dest_url, delete_flag).dimmed()
    );
    println!(); // Blank line before AzCopy output

    // Use AzCopy for the sync operation
    let azcopy = AzCopyClient::new();
    azcopy
        .sync(&source_url, &dest_url, delete_destination)
        .await?;

    println!(); // Blank line after AzCopy output
    println!("{} Sync completed successfully", "✓".green());
    Ok(())
}
