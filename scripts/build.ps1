param(
    [Parameter(Mandatory = $true)]
    [string]$Source,
    [string]$Output = "a.out"
)

$Root = Split-Path -Parent $PSScriptRoot
$BuildDir = Join-Path $Root "build"
$Asm = Join-Path $BuildDir "out.s"

New-Item -ItemType Directory -Force -Path $BuildDir | Out-Null

$WslSource = ($Source -replace '^([A-Za-z]):', '/mnt/$1').Replace('\\','/')
$WslAsm = ($Asm -replace '^([A-Za-z]):', '/mnt/$1').Replace('\\','/')
$WslRoot = ($Root -replace '^([A-Za-z]):', '/mnt/$1').Replace('\\','/')

wsl.exe bash -lc "python3 $WslRoot/src/main.py $WslSource --emit $WslAsm"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

wsl.exe bash -lc "gcc $WslAsm $WslRoot/runtime/runtime.c -rdynamic -o $Output"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Built: $Output"
