# Dot-source this file, then run: Enable-RtlCommand -Name codex, gemini
# Does not edit your profile or enable any wrappers until explicitly called.
function Enable-RtlCommand {
    param(
        [Parameter(Mandatory = $true)]
        [ValidatePattern('^[A-Za-z0-9_-]+$')]
        [string[]] $Name
    )
    $rtlExecutable = (Get-Command rtl -CommandType Application -ErrorAction Stop).Source
    foreach ($agent in $Name) {
        if ($agent -in @('rtl', 'Enable-RtlCommand')) {
            throw "Cannot wrap $agent"
        }
        if (Get-Command $agent -CommandType Function, Alias -ErrorAction SilentlyContinue) {
            throw "$agent is already a function or alias; leaving it unchanged"
        }
        $agentExecutable = (Get-Command $agent -CommandType Application -ErrorAction Stop).Source
        $wrapper = {
            if ($env:RTL_ACTIVE) {
                & $agentExecutable @args
            } else {
                & $rtlExecutable $agentExecutable @args
            }
        }.GetNewClosure()
        Set-Item -Path "Function:global:$agent" -Value $wrapper
    }
}

