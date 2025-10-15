# azst installation script for Windows PowerShell
# Usage: irm https://raw.githubusercontent.com/dymaxionlabs/azst/main/install.ps1 | iex

$ErrorActionPreference = 'Stop'

$Repo = "dymaxionlabs/azst"
$BinaryName = "azst"
$InstallDir = if ($env:AZST_INSTALL_DIR) { $env:AZST_INSTALL_DIR } else { "$env:LOCALAPPDATA\Programs\azst" }

# Colors for output
function Write-ColorOutput {
    param(
        [string]$Message,
        [string]$Color = "White"
    )
    Write-Host $Message -ForegroundColor $Color
}

Write-ColorOutput "azst installer" Green
Write-Host "Installing latest build from main branch"
Write-Host ""

# Detect architecture
$Arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { "x86" }
if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") {
    $Arch = "aarch64"
}

# Construct download URL
$ArchiveName = "${BinaryName}-windows-${Arch}.exe.zip"
$LatestVersion = "latest"
$DownloadUrl = "https://github.com/${Repo}/releases/download/${LatestVersion}/${ArchiveName}"

Write-Host "Downloading from: $DownloadUrl"

# Create temporary directory
$TmpDir = Join-Path $env:TEMP "azst-install-$(Get-Random)"
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

try {
    # Download archive
    $ArchivePath = Join-Path $TmpDir $ArchiveName
    try {
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $ArchivePath -UseBasicParsing
    }
    catch {
        Write-ColorOutput "Error: Failed to download $ArchiveName" Red
        Write-ColorOutput "Error details: $_" Red
        exit 1
    }

    # Extract archive
    Write-Host "Extracting..."
    Expand-Archive -Path $ArchivePath -DestinationPath $TmpDir -Force

    # Create installation directory if it doesn't exist
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    # Install binary
    $BinaryFile = "${BinaryName}.exe"
    $SourcePath = Join-Path $TmpDir $BinaryFile
    $DestPath = Join-Path $InstallDir $BinaryFile

    Write-Host "Installing to $DestPath..."
    Copy-Item -Path $SourcePath -Destination $DestPath -Force

    # Check if install directory is in PATH
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($UserPath -notlike "*$InstallDir*") {
        Write-Host ""
        Write-ColorOutput "Adding $InstallDir to PATH..." Yellow
        [Environment]::SetEnvironmentVariable(
            "Path",
            "$UserPath;$InstallDir",
            "User"
        )
        # Update PATH for current session
        $env:Path += ";$InstallDir"
        Write-ColorOutput "✓ PATH updated" Green
        Write-Host ""
        Write-ColorOutput "Note: You may need to restart your terminal for PATH changes to take effect." Yellow
    }

    # Download and install AzCopy
    Write-Host ""
    Write-Host "Checking for AzCopy..."

    $AzCopyVersion = "10.30.1"
    $AzCopyDir = Join-Path $env:LOCALAPPDATA "Programs\azst\azcopy"
    $AzCopyBinary = Join-Path $AzCopyDir "azcopy.exe"

    # Check if we already have the correct AzCopy version
    $AzCopyNeedsInstall = $true
    if (Test-Path $AzCopyBinary) {
        try {
            $CurrentVersion = (& $AzCopyBinary --version 2>$null | Select-Object -First 1).Split()[2]
            if ($CurrentVersion -eq $AzCopyVersion) {
                Write-Host "AzCopy $AzCopyVersion already installed"
                $AzCopyNeedsInstall = $false
            }
        }
        catch {
            # If version check fails, we'll reinstall
        }
    }

    # Install AzCopy if needed
    if ($AzCopyNeedsInstall) {
        Write-Host "Installing AzCopy $AzCopyVersion..."

        # Determine AzCopy download URL based on architecture
        $AzCopyUrl = switch ($Arch) {
            "x86_64" { "https://github.com/Azure/azure-storage-azcopy/releases/download/v${AzCopyVersion}/azcopy_windows_amd64_${AzCopyVersion}.zip" }
            "aarch64" { "https://github.com/Azure/azure-storage-azcopy/releases/download/v${AzCopyVersion}/azcopy_windows_arm64_${AzCopyVersion}.zip" }
            default {
                Write-ColorOutput "Warning: Unsupported architecture $Arch for AzCopy. You may need to install AzCopy manually." Yellow
                $null
            }
        }

        if ($AzCopyUrl) {
            # Create AzCopy directory
            if (-not (Test-Path $AzCopyDir)) {
                New-Item -ItemType Directory -Path $AzCopyDir -Force | Out-Null
            }

            # Download AzCopy
            $AzCopyArchiveName = Split-Path $AzCopyUrl -Leaf
            $AzCopyArchivePath = Join-Path $TmpDir $AzCopyArchiveName

            Write-Host "Downloading AzCopy from: $AzCopyUrl"
            try {
                Invoke-WebRequest -Uri $AzCopyUrl -OutFile $AzCopyArchivePath -UseBasicParsing

                # Extract AzCopy
                $AzCopyExtractDir = Join-Path $TmpDir "azcopy-extract"
                Expand-Archive -Path $AzCopyArchivePath -DestinationPath $AzCopyExtractDir -Force

                # Find the azcopy.exe binary (it's usually in a subdirectory)
                $AzCopyExtracted = Get-ChildItem -Path $AzCopyExtractDir -Filter "azcopy.exe" -Recurse | Select-Object -First 1

                if ($AzCopyExtracted) {
                    # Install the binary
                    Copy-Item -Path $AzCopyExtracted.FullName -Destination $AzCopyBinary -Force
                    Write-ColorOutput "✓ AzCopy $AzCopyVersion installed successfully" Green
                }
                else {
                    Write-ColorOutput "Warning: Could not find azcopy.exe in downloaded archive" Yellow
                }
            }
            catch {
                Write-ColorOutput "Warning: Failed to download AzCopy. You may need to install it manually from https://aka.ms/downloadazcopy" Yellow
            }
        }
    }

    Write-Host ""
    Write-ColorOutput "✓ Installation complete!" Green
    Write-Host ""
    Write-Host "Run 'azst --help' to get started"

    # Verify installation
    try {
        $Version = & $DestPath --version 2>$null
        if ($Version) {
            Write-Host ""
            Write-Host "Installed version:"
            Write-Host $Version
        }
    }
    catch {
        Write-Host ""
        Write-ColorOutput "Note: You may need to restart your terminal for PATH changes to take effect." Yellow
    }
}
finally {
    # Cleanup
    if (Test-Path $TmpDir) {
        Remove-Item -Path $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
