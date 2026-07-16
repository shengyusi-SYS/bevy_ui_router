$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$workspaceRoot = Split-Path -Parent $PSScriptRoot
$cataloguePath = Join-Path $workspaceRoot 'crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs'
$authorityPrefix = 'authority_regression_tests::'

function Fail([string] $Message) {
    throw "authority regression verification failed: $Message"
}

function Read-Catalogue {
    $source = Get-Content -Raw -LiteralPath $cataloguePath
    $block = [regex]::Match(
        $source,
        '(?s)pub const AUTHORITY_REGRESSION_CATALOGUE:\s*&\[&str\]\s*=\s*&\[(?<body>.*?)\];'
    )
    if (-not $block.Success) {
        Fail 'catalogue constant was not found'
    }

    $body = $block.Groups['body'].Value
    $matches = [regex]::Matches($body, '"(?<name>[A-Za-z_][A-Za-z0-9_]*)"')
    $residue = [regex]::Replace($body, '"[A-Za-z_][A-Za-z0-9_]*"', '')
    $residue = [regex]::Replace($residue, '[\s,]', '')
    if ($residue.Length -ne 0) {
        Fail "catalogue contains unparsed content: $residue"
    }

    return @($matches | ForEach-Object { $_.Groups['name'].Value })
}

function Assert-ExactSet {
    param(
        [string[]] $Expected,
        [string[]] $Actual,
        [string] $Label
    )

    if ($Expected.Count -eq 0) {
        Fail 'catalogue is empty'
    }
    if ($Actual.Count -eq 0) {
        Fail "$Label is empty"
    }

    $duplicates = @(
        $Actual |
            Group-Object |
            Where-Object Count -gt 1 |
            ForEach-Object Name
    )
    if ($duplicates.Count -ne 0) {
        Fail "$Label contains duplicates: $($duplicates -join ', ')"
    }

    $missing = @($Expected | Where-Object { $_ -notin $Actual })
    $extra = @($Actual | Where-Object { $_ -notin $Expected })
    if ($missing.Count -ne 0 -or $extra.Count -ne 0) {
        Fail "$Label differs from catalogue; missing=[$($missing -join ', ')]; extra=[$($extra -join ', ')]"
    }
}

function Invoke-CargoCapture {
    param([string[]] $Arguments)

    $previousErrorAction = $ErrorActionPreference
    Push-Location $workspaceRoot
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& cargo @Arguments 2>&1 | ForEach-Object { $_.ToString() })
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorAction
        Pop-Location
    }
    if ($exitCode -ne 0) {
        $output | ForEach-Object { Write-Host $_ }
        Fail "cargo exited with code $exitCode"
    }
    return $output
}

$catalogue = @(Read-Catalogue)
Assert-ExactSet -Expected $catalogue -Actual $catalogue -Label 'catalogue'

$cargoBase = @(
    'test',
    '--locked',
    '--config',
    "build.rustc-wrapper = ''",
    '-p',
    'bevy_ui_router_ecs',
    '--lib',
    'authority_regression_tests::'
)

$listOutput = @(Invoke-CargoCapture -Arguments ($cargoBase + @('--', '--list')))
$registered = @(
    $listOutput |
        ForEach-Object {
            if ($_ -match '^authority_regression_tests::(?<name>[A-Za-z_][A-Za-z0-9_]*): test$') {
                $Matches['name']
            }
        }
)
Assert-ExactSet -Expected $catalogue -Actual $registered -Label 'compiled registration set'

$runOutput = @(Invoke-CargoCapture -Arguments $cargoBase)
$passed = @(
    $runOutput |
        ForEach-Object {
            if ($_ -match '^test authority_regression_tests::(?<name>[A-Za-z_][A-Za-z0-9_]*) \.\.\. ok$') {
                $Matches['name']
            }
        }
)
Assert-ExactSet -Expected $catalogue -Actual $passed -Label 'actual passed set'

$authorityResults = @(
    $runOutput |
        Where-Object { $_ -match '^test authority_regression_tests::' }
)
if ($authorityResults.Count -ne $catalogue.Count) {
    Fail "catalogue tests were filtered or emitted unexpected results; expected=$($catalogue.Count); actual=$($authorityResults.Count)"
}

$summaries = @(
    $runOutput |
        ForEach-Object {
            if ($_ -match '^test result: (?<status>ok|FAILED)\. (?<passed>\d+) passed; (?<failed>\d+) failed; (?<ignored>\d+) ignored; (?<measured>\d+) measured; (?<filtered>\d+) filtered out;') {
                [pscustomobject]@{
                    Status = $Matches['status']
                    Passed = [int] $Matches['passed']
                    Failed = [int] $Matches['failed']
                    Ignored = [int] $Matches['ignored']
                    Measured = [int] $Matches['measured']
                    Filtered = [int] $Matches['filtered']
                }
            }
        }
)
if ($summaries.Count -ne 1) {
    Fail "expected one cargo result summary, found $($summaries.Count)"
}
$summary = $summaries[0]
if (
    $summary.Status -ne 'ok' -or
    $summary.Passed -ne $catalogue.Count -or
    $summary.Failed -ne 0 -or
    $summary.Ignored -ne 0 -or
    $summary.Measured -ne 0
) {
    Fail "unexpected result counts: status=$($summary.Status) passed=$($summary.Passed) failed=$($summary.Failed) ignored=$($summary.Ignored) measured=$($summary.Measured)"
}

Write-Host "authority regression verification passed: catalogue=$($catalogue.Count) registered=$($registered.Count) passed=$($passed.Count)"
