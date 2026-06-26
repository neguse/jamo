param(
    [string]$Configuration = "release",
    [switch]$Register
)

$ErrorActionPreference = "Stop"

$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
$dll = Join-Path $repo "target\$Configuration\jamo_ime.dll"
$cli = Join-Path $repo "target\$Configuration\jamo-cli.exe"

if (!(Test-Path $dll)) {
    throw "jamo_ime.dll not found: $dll"
}

$dumpbin = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
if ($dumpbin) {
    $exports = & $dumpbin.Source /exports $dll
    foreach ($name in @("DllGetClassObject", "DllCanUnloadNow", "DllRegisterServer", "DllUnregisterServer")) {
        if (($exports -join "`n") -notmatch $name) {
            throw "missing export: $name"
        }
    }
} else {
    Write-Warning "dumpbin.exe not found; skipping export inspection"
}

$buildArgs = @("build", "-p", "jamo-cli")
if ($Configuration -eq "release") {
    $buildArgs = @("build", "--release", "-p", "jamo-cli")
}
& cargo @buildArgs
if ($LASTEXITCODE -ne 0) {
    throw "cargo build jamo-cli failed with exit code $LASTEXITCODE"
}
if (!(Test-Path $cli)) {
    throw "jamo-cli.exe not found: $cli"
}

$server = Start-Process `
    -FilePath $cli `
    -ArgumentList @("serve", "--ime", "--dict", "data/ipadic") `
    -WorkingDirectory $repo `
    -WindowStyle Hidden `
    -PassThru

try {
    Start-Sleep -Seconds 3
    if ($server.HasExited) {
        throw "jamo serve --ime exited early with code $($server.ExitCode)"
    }
} finally {
    if (!$server.HasExited) {
        Stop-Process -Id $server.Id -Force
    }
}

if ($Register) {
    & regsvr32.exe /s $dll
    if ($LASTEXITCODE -ne 0) {
        throw "regsvr32 register failed with exit code $LASTEXITCODE"
    }
    try {
        & regsvr32.exe /s /u $dll
        if ($LASTEXITCODE -ne 0) {
            throw "regsvr32 unregister failed with exit code $LASTEXITCODE"
        }
    } catch {
        throw
    }
}
