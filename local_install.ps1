# Kumo Local Installer for Windows (Testing)
Write-Host "Installing Kumo Package Manager locally..." -ForegroundColor Cyan

if (!(Test-Path "Cargo.toml")) {
    Write-Error "Please run this script from the root of the Kumo repository."
    exit 1
}

$InstallDir = "$HOME\.kumo\bin"
if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
}

$Binaries = @("kumo.exe", "kx.exe")
foreach ($Bin in $Binaries) {
    $SourcePath = Join-Path "target\release" $Bin
    if (Test-Path $SourcePath) {
        $DestPath = Join-Path $InstallDir $Bin
        Write-Host "Installing $Bin to $DestPath..." -ForegroundColor Gray
        Copy-Item -Path $SourcePath -Destination $DestPath -Force
    } else {
        Write-Error "Could not find $Bin in target\release. Please run 'cargo build --release' first."
        exit 1
    }
}

Write-Host "Kumo and KX installed successfully in $InstallDir" -ForegroundColor Green

# Add to User PATH if not already there
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to User PATH..." -ForegroundColor Yellow
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    $env:Path += ";$InstallDir"
    Write-Host "PATH updated. Please restart your terminal." -ForegroundColor Cyan
} else {
    Write-Host "Kumo is already in your PATH." -ForegroundColor Green
}
