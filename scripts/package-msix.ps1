<#
Packages a Windows executable and committed app icons into an MSIX with
the Store identity, manifest, and supported locales. Converts SemVer to a
four-part MSIX version and uses the Windows SDK MakeAppx utility.
Run from build-installer CI, or invoke with -Executable, -Version, and -Output.
Recreates RUNNER_TEMP/rbxport-msix staging and writes the requested package.
#>
param(
  [Parameter(Mandatory = $true)]
  [string]$Executable,

  [Parameter(Mandatory = $true)]
  [string]$Version,

  [Parameter(Mandatory = $true)]
  [string]$Output
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) {
  throw "Application executable does not exist: $Executable"
}

# MSIX versions have exactly four numeric components. Release tags remain
# SemVer; prerelease labels intentionally do not become part of package
# identity, while the fourth component leaves room for future rebuilds.
$match = [regex]::Match($Version, '^(\d+)\.(\d+)\.(\d+)(?:-[0-9A-Za-z.-]+)?$')
if (-not $match.Success) {
  throw "Version must be SemVer: $Version"
}
$components = 1..3 | ForEach-Object { [uint32]$match.Groups[$_].Value }
if ($components | Where-Object { $_ -gt 65535 }) {
  throw "MSIX version components must be between 0 and 65535: $Version"
}
$msixVersion = '{0}.{1}.{2}.0' -f $match.Groups[1].Value, $match.Groups[2].Value, $match.Groups[3].Value

$windowsKits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
$makeAppx = Get-ChildItem -LiteralPath $windowsKits -Filter makeappx.exe -Recurse -File |
  Where-Object { $_.Directory.Name -eq 'x64' } |
  Sort-Object { [version]$_.Directory.Parent.Name } -Descending |
  Select-Object -First 1
if (-not $makeAppx) {
  throw "makeappx.exe was not found under $windowsKits; install the Windows 10/11 SDK."
}

$identityName = $env:RBX_MSIX_IDENTITY
$publisher = $env:RBX_MSIX_PUBLISHER
$publisherDisplayName = $env:RBX_MSIX_PUBLISHER_NAME
if (-not $identityName -or -not $publisher -or -not $publisherDisplayName) {
  throw "Configure RBX_MSIX_IDENTITY, RBX_MSIX_PUBLISHER and RBX_MSIX_PUBLISHER_NAME for your own package."
}

$staging = Join-Path $env:RUNNER_TEMP 'rbxport-msix'
if (Test-Path -LiteralPath $staging) {
  Remove-Item -LiteralPath $staging -Recurse -Force
}
$assets = Join-Path $staging 'Assets'
New-Item -ItemType Directory -Path $assets -Force | Out-Null

Copy-Item -LiteralPath $Executable -Destination (Join-Path $staging 'rbxport.exe')
$iconDirectory = Join-Path $PSScriptRoot '..\src-tauri\icons'
foreach ($icon in 'StoreLogo.png', 'Square44x44Logo.png', 'Square150x150Logo.png') {
  Copy-Item -LiteralPath (Join-Path $iconDirectory $icon) -Destination (Join-Path $assets $icon)
}

$manifest = @"
<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap rescap">
  <Identity Name="$identityName" Publisher="$publisher" Version="$msixVersion" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>rbxport</DisplayName>
    <PublisherDisplayName>$publisherDisplayName</PublisherDisplayName>
    <Description>DJ library and USB export manager</Description>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Resources>
    <Resource Language="en-us" />
    <Resource Language="fr-fr" />
    <Resource Language="de-de" />
    <Resource Language="es-es" />
    <Resource Language="it-it" />
    <Resource Language="nl-nl" />
    <Resource Language="ru-ru" />
    <Resource Language="pt-pt" />
    <Resource Language="sv-se" />
    <Resource Language="da-dk" />
    <Resource Language="tr-tr" />
    <Resource Language="el-gr" />
    <Resource Language="hu-hu" />
    <Resource Language="cs-cz" />
    <Resource Language="zh-cn" />
    <Resource Language="zh-tw" />
    <Resource Language="ko-kr" />
    <Resource Language="ja-jp" />
  </Resources>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Applications>
    <Application Id="rbxport" Executable="rbxport.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements
        DisplayName="rbxport"
        Description="DJ library and USB export manager"
        BackgroundColor="transparent"
        Square44x44Logo="Assets\Square44x44Logo.png"
        Square150x150Logo="Assets\Square150x150Logo.png" />
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
</Package>
"@
[System.IO.File]::WriteAllText((Join-Path $staging 'AppxManifest.xml'), $manifest, [System.Text.UTF8Encoding]::new($false))

$outputDirectory = Split-Path -Parent $Output
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
& $makeAppx.FullName pack /o /v /h SHA256 /d $staging /p $Output
if ($LASTEXITCODE -ne 0) {
  throw "MakeAppx failed with exit code $LASTEXITCODE"
}

Write-Host "Created $Output (identity $identityName, version $msixVersion, publisher $publisher)"
