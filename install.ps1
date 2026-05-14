# Kumo Installer for Windows
Write-Host "Installing Kumo Package Manager..." -ForegroundColor Cyan

$InstallDir = "$HOME\.kumo\bin"
$TempZip = "$env:TEMP\kumo-windows.zip"
$TempExtract = "$env:TEMP\kumo_extract"

if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
}

Write-Host "Downloading latest Kumo release..." -ForegroundColor Cyan
$ReleaseUrl = "https://github.com/jmaxdev/Kumo/releases/latest/download/kumo-windows.zip"
Invoke-WebRequest -Uri $ReleaseUrl -OutFile $TempZip

Write-Host "Extracting..." -ForegroundColor Gray
if (Test-Path $TempExtract) { Remove-Item -Recurse -Force $TempExtract }
Expand-Archive -Path $TempZip -DestinationPath $TempExtract -Force

# Find and move binaries regardless of structure
Get-ChildItem -Path $TempExtract -Filter "*.exe" -Recurse | ForEach-Object {
    $DestPath = Join-Path $InstallDir $_.Name
    Write-Host "Installing $($_.Name)..." -ForegroundColor Gray
    Move-Item -Path $_.FullName -Destination $DestPath -Force
}

# Cleanup
Remove-Item $TempZip -Force
Remove-Item -Recurse $TempExtract -Force

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
