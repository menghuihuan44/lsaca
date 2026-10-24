<#
.SYNOPSIS
    Build and run the C17 runtime smoke test (runtime/test/main.c).

.DESCRIPTION
    The repository path contains non-ASCII characters, so the runtime sources are
    copied into an ASCII temporary directory before compiling. Three builds are
    produced:

        smoke_release.exe   plain -O1 build
        smoke_debug.exe     -DLSACA_DEBUG (checked arithmetic + leak report)
        smoke_rc.exe        -DLSACA_DEBUG_RC (leak report only)

    Every build must compile warning free under -std=c17 -Wall -Wextra
    -Wpedantic. The script then checks:

      * the in-process suite passes and prints ALL CHECKS PASSED with no stderr
        output (in particular the debug builds must report no leaked objects),
      * --leak intentionally leaks two objects and is reported only by the
        debug / rc builds,
      * every panic mode exits 101 and prints the exact Rust-style message,
      * a double release is detected by the runtime,
      * --args, --stdin, --exit-42, --io and an unknown mode behave as specified.

    The script exits 0 only when every check passes. It is not part of the
    generated CMake project.

.EXAMPLE
    pwsh -File runtime/test/run_smoke.ps1
#>

[CmdletBinding()]
param()

$ErrorActionPreference = 'Continue'

$script:Checks = 0
$script:Failures = New-Object System.Collections.Generic.List[string]
$script:RunCounter = 0

$script:TestDir = $PSScriptRoot
$script:RuntimeDir = Split-Path -Parent $script:TestDir
$script:RepoRoot = Split-Path -Parent $script:RuntimeDir
$script:WorkDir = Join-Path $env:TEMP 'lsaca_rt_smoke'

function Assert-True {
    param([bool]$Condition, [string]$Message)
    $script:Checks += 1
    if (-not $Condition) {
        $script:Failures.Add($Message)
        Write-Host "FAIL: $Message" -ForegroundColor Red
    }
}

function Assert-Equal {
    param($Actual, $Expected, [string]$Message)
    if ($Actual -eq $Expected) {
        Assert-True $true $Message
    } else {
        $display = ($Actual | Out-String).TrimEnd()
        if ($display.Length -gt 400) { $display = $display.Substring(0, 400) + '...' }
        Assert-True $false "$Message`n  expected: $Expected`n  actual:   $display"
    }
}

function Assert-Contains {
    param([string]$Text, [string]$Needle, [string]$Message)
    Assert-True ($Text.Contains($Needle)) "$Message (output does not contain '$Needle')"
}

function Invoke-Program {
    param(
        [Parameter(Mandatory = $true)][string]$Exe,
        [string[]]$Arguments = @(),
        [string]$StdinFile = ''
    )
    # Start-Process with redirected standard handles is rejected by some
    # sandboxed hosts, so the child is started through cmd.exe with file
    # redirection. cmd.exe propagates the child exit code, keeps stdout and
    # stderr apart and feeds the exact bytes of StdinFile, which matters for
    # the CRLF handling of read_line.
    $script:RunCounter += 1
    $outFile = Join-Path $script:WorkDir ("run{0}.out" -f $script:RunCounter)
    $errFile = Join-Path $script:WorkDir ("run{0}.err" -f $script:RunCounter)
    $parts = @('"' + $Exe + '"')
    foreach ($argument in $Arguments) {
        $parts += '"' + $argument + '"'
    }
    $commandLine = $parts -join ' '
    if ($StdinFile -ne '') {
        $commandLine = $commandLine + ' < "' + $StdinFile + '"'
    }
    $commandLine = $commandLine + ' > "' + $outFile + '" 2> "' + $errFile + '"'
    & cmd.exe /d /c $commandLine
    $exitCode = $LASTEXITCODE
    $stdout = ''
    $stderr = ''
    if (Test-Path -LiteralPath $outFile) { $stdout = [System.IO.File]::ReadAllText($outFile) }
    if (Test-Path -LiteralPath $errFile) { $stderr = [System.IO.File]::ReadAllText($errFile) }
    return [pscustomobject]@{
        ExitCode = $exitCode
        StdOut   = $stdout
        StdErr   = $stderr
    }
}

function Find-Gcc {
    $candidate = 'C:\msys64\ucrt64\bin\gcc.exe'
    if (Test-Path -LiteralPath $candidate) {
        $env:PATH = (Split-Path -Parent $candidate) + ';' + $env:PATH
        return $candidate
    }
    $command = Get-Command 'gcc' -ErrorAction SilentlyContinue
    if ($null -ne $command) { return $command.Source }
    throw 'gcc not found: install it or put it on PATH'
}

function Build-Config {
    param(
        [string]$Name,
        [string[]]$Defines
    )
    $exePath = Join-Path $script:WorkDir ("smoke_{0}.exe" -f $Name)
    $arguments = @('-std=c17', '-Wall', '-Wextra', '-Wpedantic', '-O1')
    $arguments += $Defines
    $arguments += @('-o', $exePath, $script:MainC, $script:RtC, '-lm')
    $compileOutput = (& $script:Gcc @arguments 2>&1 | Out-String).Trim()
    Assert-Equal $LASTEXITCODE 0 "gcc build '$Name' succeeds"
    Assert-Equal $compileOutput '' "gcc build '$Name' is warning free"
    Assert-True (Test-Path -LiteralPath $exePath) "gcc build '$Name' produced $exePath"
    return $exePath
}

Write-Host '=== lsaca runtime smoke test ==='

# --- stage sources in an ASCII directory ---------------------------------
if (Test-Path -LiteralPath $script:WorkDir) {
    Remove-Item -LiteralPath $script:WorkDir -Recurse -Force
}
New-Item -ItemType Directory -Path $script:WorkDir | Out-Null
$script:MainC = Join-Path $script:WorkDir 'main.c'
$script:RtC = Join-Path $script:WorkDir 'lsaca_rt.c'
Copy-Item -LiteralPath (Join-Path $script:RuntimeDir 'lsaca_rt.h') -Destination $script:WorkDir
Copy-Item -LiteralPath (Join-Path $script:RuntimeDir 'lsaca_rt.c') -Destination $script:RtC
Copy-Item -LiteralPath (Join-Path $script:TestDir 'main.c') -Destination $script:MainC

$script:Gcc = Find-Gcc
Write-Host "compiler: $script:Gcc"
Write-Host "build dir: $script:WorkDir"

# --- builds ---------------------------------------------------------------
$release = Build-Config 'release' @()
$debug = Build-Config 'debug' @('-DLSACA_DEBUG')
$rc = Build-Config 'rc' @('-DLSACA_DEBUG_RC')

# --- in-process suite -----------------------------------------------------
foreach ($config in @(
        @{ Name = 'release'; Exe = $release },
        @{ Name = 'debug'; Exe = $debug },
        @{ Name = 'rc'; Exe = $rc })) {
    $result = Invoke-Program -Exe $config.Exe
    Assert-Equal $result.ExitCode 0 "$($config.Name): default suite exits 0"
    Assert-Contains $result.StdOut 'ALL CHECKS PASSED' "$($config.Name): default suite passes"
    Assert-Equal $result.StdErr '' "$($config.Name): default suite writes nothing to stderr (no leaks)"
}

# --- leak reporting -------------------------------------------------------
$result = Invoke-Program -Exe $release -Arguments @('--leak')
Assert-Equal $result.ExitCode 0 'release: --leak exits 0'
Assert-Equal $result.StdErr '' 'release: leaked objects are not reported'

$leakCases = @(
    @{ Name = 'debug'; Exe = $debug },
    @{ Name = 'rc'; Exe = $rc }
)
foreach ($config in $leakCases) {
    $result = Invoke-Program -Exe $config.Exe -Arguments @('--leak')
    Assert-Equal $result.ExitCode 0 "$($config.Name): --leak exits 0"
    Assert-Contains $result.StdErr 'lsaca: warning: 2 reference-counted allocation(s) leaked' "$($config.Name): leak report counts both objects"
    Assert-Contains $result.StdErr 'leaked allocation of 10 bytes' "$($config.Name): leak report lists allocation sizes"
}

# --- panic paths ----------------------------------------------------------
$panicCases = @(
    @{ Mode = '--panic-div-zero'; Line = 3; Col = 9; Message = 'attempt to divide by zero' },
    @{ Mode = '--panic-rem-zero'; Line = 4; Col = 2; Message = 'attempt to calculate the remainder with a divisor of zero' },
    @{ Mode = '--panic-add-overflow'; Line = 5; Col = 1; Message = 'attempt to add with overflow' },
    @{ Mode = '--panic-mul-overflow'; Line = 6; Col = 3; Message = 'attempt to multiply with overflow' },
    @{ Mode = '--panic-negate-overflow'; Line = 7; Col = 4; Message = 'attempt to negate with overflow' },
    @{ Mode = '--panic-shift-overflow'; Line = 8; Col = 5; Message = 'attempt to shift left with overflow' },
    @{ Mode = '--panic-div-overflow'; Line = 9; Col = 6; Message = 'attempt to divide with overflow' },
    @{ Mode = '--panic-rem-overflow'; Line = 10; Col = 7; Message = 'attempt to calculate the remainder with overflow' },
    @{ Mode = '--panic-bounds'; Line = 11; Col = 8; Message = 'index out of bounds: the len is 3 but the index is 99' },
    @{ Mode = '--panic-explicit'; Line = 12; Col = 9; Message = 'explicit panic' }
)

foreach ($case in $panicCases) {
    $result = Invoke-Program -Exe $release -Arguments @($case.Mode)
    $expected = "thread 'main' panicked at smoke.c:$($case.Line):$($case.Col):`n$($case.Message)`n"
    Assert-Equal $result.ExitCode 101 "$($case.Mode): exits 101"
    Assert-Equal $result.StdErr $expected "$($case.Mode): exact panic message"
    Assert-Equal $result.StdOut '' "$($case.Mode): no stdout output"
}

$result = Invoke-Program -Exe $release -Arguments @('--panic-double-release')
Assert-Equal $result.ExitCode 101 'double release: exits 101'
Assert-Contains $result.StdErr 'release of an invalid or already released reference-counted object' 'double release: runtime detects the invalid release'

# --- IO -------------------------------------------------------------------
$ioPath = Join-Path $script:WorkDir 'io_test.bin'
$result = Invoke-Program -Exe $release -Arguments @('--io', $ioPath)
Assert-Equal $result.ExitCode 0 '--io: exits 0'
Assert-Contains $result.StdOut 'ALL CHECKS PASSED' '--io: file checks pass'
Assert-Equal $result.StdErr '' '--io: writes nothing to stderr'
Assert-True (Test-Path -LiteralPath $ioPath) '--io: the file was created'

$result = Invoke-Program -Exe $release -Arguments @('--io-errors', (Join-Path $script:WorkDir 'no/such/dir/file.txt'))
Assert-Equal $result.ExitCode 0 '--io-errors: missing and unwritable paths become Err values'
Assert-Contains $result.StdOut 'ALL CHECKS PASSED' '--io-errors: error path checks pass'
Assert-Equal $result.StdErr '' '--io-errors: writes nothing to stderr'

# --- process arguments ----------------------------------------------------
$result = Invoke-Program -Exe $release -Arguments @('--args', 'alpha', 'beta')
Assert-Equal $result.ExitCode 0 '--args: exits 0'
Assert-Contains $result.StdOut "len=3`n" '--args: program name is excluded, mode flag is kept'
Assert-Contains $result.StdOut "0=--args`n" '--args: args()[0] is the first argument after the executable'
Assert-Contains $result.StdOut "1=alpha`n" '--args: second user argument'
Assert-Contains $result.StdOut "2=beta`n" '--args: third user argument'
Assert-Equal $result.StdErr '' '--args: writes nothing to stderr'

# --- stdin ----------------------------------------------------------------
$stdinFile = Join-Path $script:WorkDir 'stdin.txt'
[System.IO.File]::WriteAllBytes($stdinFile, [System.Text.Encoding]::ASCII.GetBytes("hello`r`nworld`nlast"))
$result = Invoke-Program -Exe $release -Arguments @('--stdin') -StdinFile $stdinFile
Assert-Equal $result.ExitCode 0 '--stdin: exits 0'
Assert-Contains $result.StdOut "LINE:hello`n" '--stdin: strips CRLF'
Assert-Contains $result.StdOut "LINE:world`n" '--stdin: strips LF'
Assert-Contains $result.StdOut "LINE:last`n" '--stdin: last line without newline'
Assert-Contains $result.StdOut "EOF`n" '--stdin: EOF is reported'
Assert-Equal $result.StdErr '' '--stdin: writes nothing to stderr'

$emptyStdin = Join-Path $script:WorkDir 'stdin_empty.txt'
[System.IO.File]::WriteAllBytes($emptyStdin, @())
$result = Invoke-Program -Exe $release -Arguments @('--stdin') -StdinFile $emptyStdin
Assert-Equal $result.ExitCode 0 '--stdin empty: exits 0'
Assert-Equal $result.StdOut "EOF`n" '--stdin empty: immediate EOF'

# --- exit -----------------------------------------------------------------
$result = Invoke-Program -Exe $release -Arguments @('--exit-42')
Assert-Equal $result.ExitCode 42 '--exit-42: exits with 42'
Assert-Contains $result.StdOut "before exit`n" '--exit-42: flushes stdout before exiting'

# --- unknown mode ---------------------------------------------------------
$result = Invoke-Program -Exe $release -Arguments @('--bogus')
Assert-Equal $result.ExitCode 2 'unknown mode: exits 2'
Assert-Contains $result.StdErr 'unknown mode: --bogus' 'unknown mode: explains the problem on stderr'

# --- summary --------------------------------------------------------------
Write-Host ''
if ($script:Failures.Count -eq 0) {
    Write-Host ("SMOKE PASSED ({0} checks)" -f $script:Checks) -ForegroundColor Green
    exit 0
}
Write-Host ("SMOKE FAILED: {0} of {1} checks failed" -f $script:Failures.Count, $script:Checks) -ForegroundColor Red
foreach ($failure in $script:Failures) {
    Write-Host ("  - {0}" -f $failure)
}
exit 1
