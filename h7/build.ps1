param(
    [ValidateSet('Release','Debug')][string]$Configuration='Release',
    [string]$ArmGccBin='',
    [string]$BuildDirectory=''
)
$ErrorActionPreference='Stop'
if ($ArmGccBin) {
    if (!(Test-Path -LiteralPath (Join-Path $ArmGccBin 'arm-none-eabi-gcc.exe'))) { throw '指定目录中没有 ARM GCC' }
    $env:PATH="$ArmGccBin;$env:PATH"
}
foreach ($tool in @('arm-none-eabi-gcc','arm-none-eabi-objcopy','arm-none-eabi-size','cmake','ninja')) {
    if (!(Get-Command $tool -ErrorAction SilentlyContinue)) { throw "找不到工具：$tool，请配置 PATH 或 -ArmGccBin" }
}
if (!$BuildDirectory) { $BuildDirectory=Join-Path $PSScriptRoot 'build' }
& arm-none-eabi-gcc --version
if ($LASTEXITCODE) { throw 'ARM GCC 无法启动' }
& cmake -S $PSScriptRoot -B $BuildDirectory -G Ninja "-DCMAKE_BUILD_TYPE=$Configuration" '-DCMAKE_C_FLAGS_RELEASE=-O2 -DNDEBUG'
if ($LASTEXITCODE) { throw 'CMake 配置失败' }
& cmake --build $BuildDirectory --parallel
if ($LASTEXITCODE) { throw '固件编译失败' }
Write-Host "固件已生成：$BuildDirectory\firmware.hex"
