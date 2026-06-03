# TimeShards Server (Tauri dev) with pilot env: demo off, default passwords blocked.
# Usage: $env:TIMESHARDS_ADMIN_PASSWORD = "YourSecret"; .\scripts\start_server-pilot.ps1

param([switch]$SkipNpmInstall)

. (Join-Path $PSScriptRoot "_pilot-env.ps1")
& (Join-Path $PSScriptRoot "start_server.ps1") @PSBoundParameters
