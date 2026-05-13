# Kumo Installer for Windows
Write-Host "Installing Kumo Package Manager..." -ForegroundColor Cyan

$InstallDir = "$HOME\.kumo\bin"
$RepoUrl = "https://github.com/jmaxdev/Kumo/releases/latest/download"
$Filename = "kumo-windows.zip"
$ZipPath = "$env:TEMP\$Filename"

if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
}

Write-Host "Downloading $Filename..."
Invoke-WebRequest -Uri "$RepoUrl/$Filename" -OutFile $ZipPath

Write-Host "Extracting..."
Expand-Archive -Path $ZipPath -DestinationPath $InstallDir -Force

Write-Host "Kumo installed successfully in $InstallDir" -ForegroundColor Green

# Add to User PATH if not already there
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host "Adding to PATH..." -ForegroundColor Yellow
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    $env:Path += ";$InstallDir"
    Write-Host "PATH updated. Please restart your terminal." -ForegroundColor Cyan
} else {
    Write-Host "Kumo is already in your PATH." -ForegroundColor Green
}
