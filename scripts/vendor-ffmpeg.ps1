#Requires -Version 5.1
param(
    [Parameter(Mandatory = $true)]
    [string]$DestDir
)

$ErrorActionPreference = "Stop"

New-Item -ItemType Directory -Force -Path $DestDir | Out-Null
$work = Join-Path ([System.IO.Path]::GetTempPath()) ("vertify-ffmpeg-" + [guid]::NewGuid().ToString("n"))
New-Item -ItemType Directory -Force -Path $work | Out-Null

try {
    $zip = Join-Path $work "ffmpeg.zip"
    Write-Host "Downloading ffmpeg essentials (Windows)..."
    Invoke-WebRequest -UseBasicParsing -Uri "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip" -OutFile $zip
    Expand-Archive -Path $zip -DestinationPath $work -Force

    $ffmpeg = Get-ChildItem -Path $work -Recurse -Filter ffmpeg.exe | Select-Object -First 1
    if (-not $ffmpeg) { throw "ffmpeg.exe not found in the downloaded archive" }
    $bin = $ffmpeg.Directory.FullName
    Copy-Item -Force (Join-Path $bin "ffmpeg.exe") $DestDir
    Copy-Item -Force (Join-Path $bin "ffprobe.exe") $DestDir
    Get-ChildItem -Path $bin -Filter *.dll -ErrorAction SilentlyContinue | ForEach-Object {
        Copy-Item -Force $_.FullName $DestDir
    }

    $third = Join-Path $DestDir "third_party\ffmpeg"
    New-Item -ItemType Directory -Force -Path $third | Out-Null
    Get-ChildItem -Path $work -Recurse -Include LICENSE,LICENSE.txt,COPYING*,README.txt |
        Select-Object -First 8 |
        ForEach-Object { Copy-Item -Force $_.FullName $third }
    Set-Content -Path (Join-Path $third "SOURCE.txt") -Value "Windows build: https://www.gyan.dev/ffmpeg/builds/`nFFmpeg: https://ffmpeg.org`nLicense: GPL (x264). Vertify invokes ffmpeg as a separate program."
    Write-Host "Vendored ffmpeg into $DestDir"
}
finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
