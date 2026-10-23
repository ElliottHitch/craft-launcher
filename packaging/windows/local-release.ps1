$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'local-build.ps1') -SourceRoot (Join-Path $PSScriptRoot '..\..') -AppId artcraft-launcher
