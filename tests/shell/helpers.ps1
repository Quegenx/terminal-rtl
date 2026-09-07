$ErrorActionPreference = 'Stop'
$directory = Join-Path ([IO.Path]::GetTempPath()) ('rtl-shell-' + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $directory | Out-Null
$originalPath = $env:PATH
$originalActive = $env:RTL_ACTIVE
try {
    Set-Content (Join-Path $directory 'rtl.cmd') '@echo WRAPPED:%*'
    Set-Content (Join-Path $directory 'fixture-agent.cmd') '@echo DIRECT:%*'
    $env:PATH = $directory + ';' + $env:PATH
    . (Join-Path $PSScriptRoot '../../shell/rtl.ps1')
    Set-Alias fixture-agent Write-Output
    try { Enable-RtlCommand fixture-agent; throw 'accepted alias' } catch { if ($_.Exception.Message -eq 'accepted alias') { throw } }
    Remove-Item Alias:fixture-agent
    function global:fixture-agent { 'FUNCTION' }
    try { Enable-RtlCommand fixture-agent; throw 'accepted function' } catch { if ($_.Exception.Message -eq 'accepted function') { throw } }
    Remove-Item Function:fixture-agent
    try { Enable-RtlCommand missing-fixture-command; throw 'accepted missing command' } catch { if ($_.Exception.Message -eq 'accepted missing command') { throw } }
    Enable-RtlCommand fixture-agent
    $env:RTL_ACTIVE = ''
    if ((fixture-agent 'hello') -notmatch '^WRAPPED:.*fixture-agent.cmd.*hello') { throw 'did not wrap' }
    $env:RTL_ACTIVE = '1'
    if ((fixture-agent 'hello') -ne 'DIRECT:hello') { throw 'did not bypass nested session' }
} finally {
    $env:PATH = $originalPath
    $env:RTL_ACTIVE = $originalActive
    Remove-Item Function:fixture-agent -ErrorAction SilentlyContinue
    Remove-Item $directory -Recurse -Force
}
