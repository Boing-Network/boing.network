# Replace target\release\boing-node.exe (and optional boing CLI) from an official GitHub release zip.
# Run on the **tunnel origin** (the PC where boing-node listens on the port cloudflared forwards to).
#
# Prerequisite: stop the running boing-node so the .exe is not locked.
# Cloudflare / cloudflared do **not** deploy binaries; this script only updates files on this machine.
#
# Usage (from repo root, PowerShell):
#   .\scripts\upgrade-boing-node-from-release.ps1
#   .\scripts\upgrade-boing-node-from-release.ps1 -Tag testnet-v0.1.22 -ExpectedSha256 <64-hex-of-zip>
#
param(
    [string]$Tag = "testnet-v0.1.21",
    [string]$ExpectedSha256 = "",
    [switch]$WithCli,
    [switch]$Force
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $RepoRoot

# Built-in SHA for testnet-v0.1.8 Windows zip (see website/functions/api/networks.js BOING_ZIP_SHA.windows).
$BuiltinSha = @{
    "testnet-v0.1.21" = "6b15c32deee46a6e17b252fac0050a992994c96c1447c930013f180bb4081a5f"
    "testnet-v0.1.8" = "2cea7a6f093990c02bf405a20caf3b68bb59b434b69421449ab6bb4fec96a16a"
}

$ZipName = "release-windows-x86_64.zip"
$NodeInZip = "boing-node-windows-x86_64.exe"
$CliInZip = "boing-windows-x86_64.exe"

$expect = $ExpectedSha256
if (-not $expect) {
    if ($BuiltinSha.ContainsKey($Tag)) {
        $expect = $BuiltinSha[$Tag]
    } else {
        Write-Error "No built-in SHA for tag '$Tag'. Pass -ExpectedSha256 (full zip SHA256) or use a pinned tag."
    }
}

$url = "https://github.com/Boing-Network/boing.network/releases/download/$Tag/$ZipName"
$stage = Join-Path ([System.IO.Path]::GetTempPath()) ("boing-upgrade-" + [Guid]::NewGuid().ToString("n"))
New-Item -ItemType Directory -Path $stage | Out-Null
$zipPath = Join-Path $stage $ZipName

try {
    if (-not $Force) {
        Write-Host "This will replace $RepoRoot\target\release\boing-node.exe (tag: $Tag)."
        Write-Host "Stop boing-node first. Press Enter to continue."
        Read-Host | Out-Null
    }

    Write-Host "Downloading $url ..."
    Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing

    $hash = (Get-FileHash -Algorithm SHA256 -Path $zipPath).Hash.ToLowerInvariant()
    if ($hash -ne $expect.ToLowerInvariant()) {
        Write-Error "SHA256 mismatch for $ZipName`n  expected: $expect`n  actual:   $hash"
    }

    $extract = Join-Path $stage "extract"
    New-Item -ItemType Directory -Path $extract | Out-Null
    Expand-Archive -LiteralPath $zipPath -DestinationPath $extract -Force

    $nodeSrc = Join-Path $extract $NodeInZip
    if (-not (Test-Path -LiteralPath $nodeSrc)) {
        Write-Error "Zip did not contain $NodeInZip"
    }

    $targetDir = Join-Path $RepoRoot "target\release"
    New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
    $dest = Join-Path $targetDir "boing-node.exe"
    if (Test-Path -LiteralPath $dest) {
        $bak = "$dest.bak." + (Get-Date -Format "yyyyMMddHHmmss")
        Copy-Item -LiteralPath $dest -Destination $bak -Force
    }
    Copy-Item -LiteralPath $nodeSrc -Destination $dest -Force
    Write-Host "Installed: $dest"

    if ($WithCli) {
        $cliSrc = Join-Path $extract $CliInZip
        if (-not (Test-Path -LiteralPath $cliSrc)) {
            Write-Warning "Zip did not contain $CliInZip"
        } else {
            $cliDest = Join-Path $targetDir "boing.exe"
            if (Test-Path -LiteralPath $cliDest) {
                $bakc = "$cliDest.bak." + (Get-Date -Format "yyyyMMddHHmmss")
                Copy-Item -LiteralPath $cliDest -Destination $bakc -Force
            }
            Copy-Item -LiteralPath $cliSrc -Destination $cliDest -Force
            Write-Host "Installed: $cliDest"
        }
    }
}
finally {
    Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "Done. Restart boing-node (e.g. scripts\start-bootnode-1.bat). cloudflared unchanged if RPC stays on 8545."
