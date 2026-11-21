param(
    [Parameter(Mandatory)]
    [string]$WixBin,

    [ValidateSet('x86_64-pc-windows-msvc', 'x86_64-pc-windows-gnu')]
    [string]$Target = 'x86_64-pc-windows-msvc'
)

$ErrorActionPreference = 'Stop'
$wixDirectory = (Resolve-Path -LiteralPath $WixBin).Path
$candle = Join-Path $wixDirectory 'candle.exe'
$light = Join-Path $wixDirectory 'light.exe'
foreach ($tool in @($candle, $light)) {
    if (-not (Test-Path -LiteralPath $tool -PathType Leaf)) {
        throw "Ferramenta do WiX não encontrada em $tool"
    }
}

Push-Location $PSScriptRoot
try {
    cargo build --locked --release --target $Target
    if ($LASTEXITCODE -ne 0) {
        throw "Falha na compilação Rust ($LASTEXITCODE)."
    }

    $exe = (Resolve-Path -LiteralPath ".\target\$Target\release\monitor-de-ram.exe").Path
    $rustRoot = rustc --print sysroot
    if ($LASTEXITCODE -ne 0) {
        throw "Não foi possível encontrar a instalação do Rust ($LASTEXITCODE)."
    }
    $rustNotices = (Resolve-Path -LiteralPath (Join-Path $rustRoot 'share\doc\rust\COPYRIGHT-library.html')).Path
    $versionLine = Get-Content -LiteralPath '.\Cargo.toml' |
        Select-String -Pattern '^version\s*=\s*"(\d+\.\d+\.\d+)"$' |
        Select-Object -First 1
    $version = $versionLine.Matches[0].Groups[1].Value
    $folder = (New-Item -ItemType Directory -Path '.\dist' -Force).FullName
    $object = Join-Path $folder 'monitor-de-ram.wixobj'
    $filename = "Monitor-de-RAM-$version-x64.msi"
    $msi = Join-Path $folder $filename
    & $candle -nologo -arch x64 "-dAppExe=$exe" "-dAppVersion=$version" "-dRustNotices=$rustNotices" -out $object '.\installer\monitor-de-ram.wxs'
    if ($LASTEXITCODE -ne 0) {
        throw "Falha na compilação WiX ($LASTEXITCODE)."
    }
    & $light -nologo -out $msi $object
    if ($LASTEXITCODE -ne 0) {
        throw "Falha na vinculação WiX ($LASTEXITCODE)."
    }

    $hash = (Get-FileHash -LiteralPath $msi -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $filename" | Set-Content -LiteralPath "$msi.sha256" -Encoding ascii
    Get-Item -LiteralPath $msi, "$msi.sha256"
}
finally {
    Pop-Location
}
