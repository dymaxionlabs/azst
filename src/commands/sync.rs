use anyhow::{anyhow, Result};
use colored::*;
use std::io::{self, Write};

use crate::azure::{convert_az_uri_to_url, AzCopyClient, AzCopyOptions};
use crate::utils::{is_azure_uri, parse_azure_uri};

pub struct SyncOptions<'a> {
    pub source: &'a str,
    pub destination: &'a str,
    pub delete_destination: bool,
    pub force: bool,
    pub dry_run: bool,
    pub cap_mbps: Option<f64>,
    pub block_size_mb: Option<f64>,
    pub put_md5: bool,
    pub include_pattern: Option<&'a str>,
    pub exclude_pattern: Option<&'a str>,
}

#[allow(clippy::too_many_arguments)]
pub async fn execute(
    source: &str,
    destination: &str,
    delete_destination: bool,
    force: bool,
    dry_run: bool,
    cap_mbps: Option<f64>,
    block_size_mb: Option<f64>,
    put_md5: bool,
    include_pattern: Option<&str>,
    exclude_pattern: Option<&str>,
) -> Result<()> {
    let options = SyncOptions {
        source,
        destination,
        delete_destination,
        force,
        dry_run,
        cap_mbps,
        block_size_mb,
        put_md5,
        include_pattern,
        exclude_pattern,
    };
    execute_with_options(options).await
}

async fn execute_with_options(options: SyncOptions<'_>) -> Result<()> {
    let source = options.source;
    let destination = options.destination;
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
    sync_with_azcopy(options).await
}

async fn sync_with_azcopy(options: SyncOptions<'_>) -> Result<()> {
    let source = options.source;
    let destination = options.destination;
    let delete_destination = options.delete_destination;
    let force = options.force;

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

    let mut flags_display = Vec::new();
    if delete_destination {
        flags_display.push("delete");
    }
    if options.dry_run {
        flags_display.push("dry-run");
    }
    if options.cap_mbps.is_some() {
        flags_display.push("rate-limited");
    }
    if options.block_size_mb.is_some() {
        flags_display.push("custom-block-size");
    }
    if options.put_md5 {
        flags_display.push("md5-hashing");
    }
    if options.include_pattern.is_some() {
        flags_display.push("filtered");
    }

    let flags_str = if !flags_display.is_empty() {
        format!(" ({})", flags_display.join(", "))
    } else {
        String::new()
    };

    println!(
        "{} {} {} → {}{}",
        "⇄".green(),
        operation_type,
        source.cyan(),
        destination.cyan(),
        flags_str.yellow()
    );

    // Build options
    let mut azcopy_options = AzCopyOptions::new()
        .with_dry_run(options.dry_run)
        .with_cap_mbps(options.cap_mbps)
        .with_block_size_mb(options.block_size_mb)
        .with_put_md5(options.put_md5);

    if let Some(pattern) = options.include_pattern {
        azcopy_options = azcopy_options.with_include_pattern(Some(pattern.to_string()));
    }
    if let Some(pattern) = options.exclude_pattern {
        azcopy_options = azcopy_options.with_exclude_pattern(Some(pattern.to_string()));
    }

    // Show the actual AzCopy command for debugging
    let mut cmd_parts = vec![format!("azcopy sync '{}' '{}'", source_url, dest_url)];
    if delete_destination {
        cmd_parts.push("--delete-destination=true".to_string());
    }
    if options.dry_run {
        cmd_parts.push("--dry-run".to_string());
    }
    if let Some(mbps) = options.cap_mbps {
        cmd_parts.push(format!("--cap-mbps={}", mbps));
    }
    if let Some(block_size) = options.block_size_mb {
        cmd_parts.push(format!("--block-size-mb={}", block_size));
    }
    if options.put_md5 {
        cmd_parts.push("--put-md5".to_string());
    }
    if let Some(pattern) = options.include_pattern {
        cmd_parts.push(format!("--include-pattern='{}'", pattern));
    }
    if let Some(pattern) = options.exclude_pattern {
        cmd_parts.push(format!("--exclude-pattern='{}'", pattern));
    }

    println!("{} {}", "⚙".dimmed(), cmd_parts.join(" ").dimmed());
    println!(); // Blank line before AzCopy output

    // Use AzCopy for the sync operation
    let azcopy = AzCopyClient::new();
    azcopy
        .sync_with_options(&source_url, &dest_url, delete_destination, &azcopy_options)
        .await?;

    println!(); // Blank line after AzCopy output
    println!("{} Sync completed successfully", "✓".green());
    Ok(())
}
