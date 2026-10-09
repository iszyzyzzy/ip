param([string]$HostCompiler='gcc')
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
$output=Join-Path $project 'build'
New-Item -ItemType Directory -Force $output | Out-Null
& $HostCompiler -std=c11 -O2 -Wall -Wextra -Werror "-I$project\Core\Inc" "$PSScriptRoot\test_protocol.c" "$project\Core\Src\protocol.c" -o "$output\test_protocol.exe"
if ($LASTEXITCODE) { throw 'C 协议检查程序编译失败' }
& "$output\test_protocol.exe"
if ($LASTEXITCODE) { throw 'C 协议检查失败' }
& $HostCompiler -std=c11 -O2 -Wall -Wextra -Werror "-I$project\Core\Inc" "$PSScriptRoot\test_motor.c" "$project\Core\Src\motor.c" "$project\Core\Src\protocol.c" -lm -o "$output\test_motor.exe"
if ($LASTEXITCODE) { throw '电机控制检查程序编译失败' }
& "$output\test_motor.exe"
if ($LASTEXITCODE) { throw '电机控制检查失败' }
& $HostCompiler -std=c11 -O2 -Wall -Wextra -Werror "-I$PSScriptRoot\hal_stub" "-I$project\Core\Inc" "$PSScriptRoot\test_motor_can.c" "$project\Core\Src\motor_can.c" -o "$output\test_motor_can.exe"
if ($LASTEXITCODE) { throw '电机 CAN 适配检查程序编译失败' }
& "$output\test_motor_can.exe"
if ($LASTEXITCODE) { throw '电机 CAN 适配检查失败' }
& python "$PSScriptRoot\test_host.py" -v
if ($LASTEXITCODE) { throw 'Python 上位机检查失败' }
