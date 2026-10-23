param(
  [Parameter(Mandatory)] [string] $SourceRoot,
  [Parameter(Mandatory)]
  [ValidateSet('artcraft-launcher', 'photocraft', 'vectorcraft', 'filmcraft', 'lightcraft',
    'pdfcraft', 'effectcraft', 'designcraft', 'figcraft', 'wordcraft', 'soundcraft',
    'deckcraft', 'cadcraft', 'gridcraft')]
  [string] $AppId
)
$ErrorActionPreference = 'Stop'
$SourceRoot = (Resolve-Path -LiteralPath $SourceRoot).Path
$repoName = if ($AppId -eq 'artcraft-launcher') { 'craft-launcher' } else { $AppId }
if ($env:LOCAL_RELEASE_REPOSITORY -and $env:LOCAL_RELEASE_REPOSITORY -cne "ElliottHitch/$repoName") {
  throw 'The selected repository does not match this Craft package.'
}

$inPackage = $false
$version = $null
foreach ($line in Get-Content -LiteralPath (Join-Path $SourceRoot 'Cargo.toml')) {
  if ($line -match '^\s*\[') { $inPackage = $line.Trim() -eq '[workspace.package]'; continue }
  if ($inPackage -and $line -match '^\s*version\s*=\s*"([^"]+)"') { $version = $Matches[1]; break }
}
if ($version -notmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
  throw 'Craft Windows releases require a numeric workspace version.'
}
$numericVersion = [version]$version
if ($numericVersion.Major -gt 255 -or $numericVersion.Minor -gt 255 -or $numericVersion.Build -gt 65535) {
  throw 'The version exceeds Windows Installer limits.'
}
if ($env:LOCAL_RELEASE_VERSION -and $env:LOCAL_RELEASE_VERSION -cne $version) {
  throw 'The requested release version must match Cargo.toml.'
}

$dotnet = Join-Path $env:USERPROFILE '.dotnet'
$env:PATH = "$dotnet;$dotnet\tools;$env:PATH"
if (Test-Path -LiteralPath (Join-Path $dotnet 'dotnet.exe')) { $env:DOTNET_ROOT = $dotnet }
$sdk = Get-ChildItem -LiteralPath "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Directory |
  Where-Object { $_.Name -match '^10\.[0-9.]+$' -and (Test-Path -LiteralPath (Join-Path $_.FullName 'x64\rc.exe')) } |
  Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
if (-not $sdk) { throw 'Install the Windows SDK resource compiler before packaging.' }
$env:PATH = (Join-Path $sdk.FullName 'x64') + ';' + $env:PATH
foreach ($tool in 'cargo', 'rustc', 'git', 'wix') { Get-Command $tool -ErrorAction Stop | Out-Null }
$wixVersion = (& wix --version | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $wixVersion -notmatch '^5\.0\.2(?:\.|\+|$)') {
  throw 'Install WiX 5.0.2 and its UI and Util extensions before packaging.'
}

$cache = $env:CRAFT_BUILD_CACHE
if (-not $cache) { $cache = [Environment]::GetEnvironmentVariable('CRAFT_BUILD_CACHE', 'User') }
if (-not $cache) { $cache = Join-Path $env:LOCALAPPDATA 'ArtCraft\build-cache' }
$cache = [IO.Path]::GetFullPath($cache)
New-Item -ItemType Directory -Force -Path $cache | Out-Null
$env:CARGO_TARGET_DIR = $cache
$env:CARGO_INCREMENTAL = '0'
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '6' }
$env:CARGO_HTTP_USER_AGENT = "ArtCraft-Launcher/$version"
$env:DIST = Join-Path $SourceRoot 'dist\release'

function Get-PinnedInput([string] $Name, [string] $Revision) {
  if ($Revision -notmatch '^[0-9a-f]{40}$') { throw "$Name needs a full pinned commit SHA." }
  $path = Join-Path $cache "inputs\$Name-$Revision"
  if (-not (Test-Path -LiteralPath (Join-Path $path '.git'))) {
    New-Item -ItemType Directory -Force -Path $path | Out-Null
    & git -C $path init --quiet
    if ($LASTEXITCODE -ne 0) { throw "Could not initialize $Name." }
    & git -C $path remote add origin "https://github.com/storytold/$Name.git"
    if ($LASTEXITCODE -ne 0) { throw "Could not configure $Name." }
  }
  $actual = & git -C $path rev-parse --verify HEAD 2>$null
  if ($LASTEXITCODE -ne 0 -or $actual -cne $Revision) {
    & git -c "http.userAgent=ArtCraft-Launcher/$version" -C $path fetch --depth 1 origin $Revision
    if ($LASTEXITCODE -ne 0) { throw "Could not fetch pinned $Name." }
    & git -C $path checkout --detach $Revision
    if ($LASTEXITCODE -ne 0) { throw "Could not check out pinned $Name." }
  }
  $actual = & git -C $path rev-parse HEAD
  if ($LASTEXITCODE -ne 0 -or $actual -cne $Revision) { throw "$Name revision verification failed." }
  & git -C $path diff --quiet HEAD --
  if ($LASTEXITCODE -ne 0) { throw "Cached $Name contains modified tracked files." }
  return $path
}

$fontsRevision = 'bfcbf714e45a3ea8e98f88ab2b7971372a576414'
$upstreamRecipe = Join-Path $SourceRoot '.github\workflows\release.yml'
if (Test-Path -LiteralPath $upstreamRecipe) {
  $match = [regex]::Match((Get-Content -LiteralPath $upstreamRecipe -Raw), '(?m)^\s*craft-fonts-ref:\s*([0-9a-f]{40})\s*$')
  if ($match.Success) { $fontsRevision = $match.Groups[1].Value }
}
$env:CRAFT_FONTS_DIR = Get-PinnedInput 'craft-fonts' $fontsRevision
$env:CRAFT_FONTS_REQUIRED = '1'
$i18nPin = Join-Path $SourceRoot 'craft-i18n.rev'
if (Test-Path -LiteralPath $i18nPin) {
  $env:CRAFT_I18N_DIR = Get-PinnedInput 'craft-i18n' (Get-Content -LiteralPath $i18nPin -Raw).Trim()
  $env:CRAFT_I18N_REQUIRED = '1'
}

$prefix = $AppId.ToUpperInvariant().Replace('-', '_')
$sourceSha = (& git -C $SourceRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Could not identify package source.' }
if ($env:LOCAL_RELEASE_SOURCE_SHA -and $env:LOCAL_RELEASE_SOURCE_SHA -cne $sourceSha) {
  throw 'The checkout differs from the selected source commit.'
}
[Environment]::SetEnvironmentVariable("${prefix}_VERSION", $version)
[Environment]::SetEnvironmentVariable("${prefix}_BUILD_SHA", $sourceSha)
[Environment]::SetEnvironmentVariable("${prefix}_BUILD_DATE", (Get-Date).ToUniversalTime().ToString('yyyy-MM-dd'))
$prepare = Join-Path $SourceRoot 'packaging\windows\personal-prepare.ps1'
Push-Location $SourceRoot
try {
  if (Test-Path -LiteralPath $prepare) { & $prepare; if (-not $?) { throw 'Package preparation failed.' } }
  & (Join-Path $SourceRoot 'packaging\windows\package.ps1') -Arch x64
  if (-not $?) { throw 'Windows packaging failed.' }
  & git diff --exit-code HEAD --
  if ($LASTEXITCODE -ne 0) { throw 'Packaging modified tracked source files.' }
} finally { Pop-Location }
