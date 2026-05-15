param(
    [Parameter(Mandatory = $true)]
    [string]$TracePath,

    [string]$TracyDir = "C:\All\Me\2026\tracy",

    [string]$OutputDir,

    [int]$Top = 30,

    [switch]$SlowFrame,

    [int]$FrameIndex = 0,

    [switch]$KeepRawEvents
)

$ErrorActionPreference = "Stop"

function Format-Ms {
    param([Int64]$Ns)
    return "{0:N3}" -f ($Ns / 1000000.0)
}

function Write-Section {
    param(
        [System.IO.StreamWriter]$Writer,
        [string]$Title
    )
    $Writer.WriteLine("")
    $Writer.WriteLine("## $Title")
    $Writer.WriteLine("")
}

function Invoke-TracyCsvExport {
    param(
        [string[]]$Arguments,
        [string]$OutFile
    )
    & $script:CsvExport @Arguments > $OutFile
    if ($LASTEXITCODE -ne 0) {
        throw "tracy-csvexport failed with exit code $LASTEXITCODE. Args: $($Arguments -join ' ')"
    }
}

$TracePath = (Resolve-Path -LiteralPath $TracePath).Path
$CsvExport = Join-Path $TracyDir "tracy-csvexport.exe"
if (!(Test-Path -LiteralPath $CsvExport)) {
    throw "Cannot find tracy-csvexport.exe at $CsvExport"
}

if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $traceBase = [System.IO.Path]::GetFileNameWithoutExtension($TracePath)
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $OutputDir = Join-Path (Join-Path (Get-Location) "util\tracy_out") "$traceBase-$stamp"
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$OutputDir = (Resolve-Path -LiteralPath $OutputDir).Path

$statsPath = Join-Path $OutputDir "stats.tsv"
$topMaxPath = Join-Path $OutputDir "top_by_max.tsv"
$topTotalPath = Join-Path $OutputDir "top_by_total.tsv"
$messagesPath = Join-Path $OutputDir "messages.tsv"
$gpuPath = Join-Path $OutputDir "gpu.tsv"
$summaryPath = Join-Path $OutputDir "summary.md"

Write-Host "Exporting Tracy statistics..."
Invoke-TracyCsvExport -Arguments @("-s", "`t", $TracePath) -OutFile $statsPath
Invoke-TracyCsvExport -Arguments @("-m", "-s", "`t", $TracePath) -OutFile $messagesPath
Invoke-TracyCsvExport -Arguments @("-g", "-s", "`t", $TracePath) -OutFile $gpuPath

$stats = Import-Csv -Delimiter "`t" -Path $statsPath
$topByMax = $stats |
    Where-Object { $_.max_ns -match '^\d+$' } |
    Sort-Object { [Int64]$_.max_ns } -Descending |
    Select-Object -First $Top
$topByTotal = $stats |
    Where-Object { $_.total_ns -match '^\d+$' } |
    Sort-Object { [Int64]$_.total_ns } -Descending |
    Select-Object -First $Top

$topByMax | Export-Csv -Delimiter "`t" -NoTypeInformation -Path $topMaxPath
$topByTotal | Export-Csv -Delimiter "`t" -NoTypeInformation -Path $topTotalPath

$frameInfo = $null
if ($SlowFrame -or $FrameIndex -gt 0) {
    if ($FrameIndex -gt 0) {
        Write-Host "Finding update frame #$FrameIndex..."
    } else {
        Write-Host "Finding slowest update zone..."
    }
    $updatesPath = Join-Path $OutputDir "updates.tsv"
    Invoke-TracyCsvExport -Arguments @("-u", "-s", "`t", "-f", "update", $TracePath) -OutFile $updatesPath

    $updates = Import-Csv -Delimiter "`t" -Path $updatesPath |
        Where-Object { $_.name -eq "update" -and $_.ns_since_start -match '^\d+$' -and $_.exec_time_ns -match '^\d+$' } |
        Sort-Object { [Int64]$_.ns_since_start }

    if ($null -eq $updates -or @($updates).Count -eq 0) {
        Write-Warning "No update zone was found. Frame export skipped."
    } else {
        if ($FrameIndex -gt 0) {
            if ($FrameIndex -gt @($updates).Count) {
                throw "FrameIndex $FrameIndex is out of range. Trace has $(@($updates).Count) update frames."
            }
            $selectedUpdate = @($updates)[$FrameIndex - 1]
            $frameLabel = "frame_$FrameIndex"
        } else {
            $selectedUpdate = $updates | Sort-Object { [Int64]$_.exec_time_ns } -Descending | Select-Object -First 1
            $frameLabel = "slowest_frame"
        }

        $frameStart = [Int64]$selectedUpdate.ns_since_start
        $frameEnd = $frameStart + [Int64]$selectedUpdate.exec_time_ns
        $frameEventsPath = Join-Path $OutputDir "$frameLabel`_events.tsv"
        $frameTopPath = Join-Path $OutputDir "$frameLabel`_top.tsv"
        $rawEventsPath = Join-Path $OutputDir "events.tsv"

        Write-Host "Exporting all CPU zone events; this can be large..."
        Invoke-TracyCsvExport -Arguments @("-u", "-s", "`t", $TracePath) -OutFile $rawEventsPath

        Write-Host "Filtering events overlapping target update frame..."
        $reader = [System.IO.StreamReader]::new($rawEventsPath)
        $writer = [System.IO.StreamWriter]::new($frameEventsPath, $false, [System.Text.UTF8Encoding]::new($false))
        try {
            $header = $reader.ReadLine()
            $writer.WriteLine($header)
            while (($line = $reader.ReadLine()) -ne $null) {
                $cols = $line -split "`t", 7
                if ($cols.Count -lt 7) { continue }
                $eventStart = 0L
                $eventDuration = 0L
                if (![Int64]::TryParse($cols[3], [ref]$eventStart)) { continue }
                if (![Int64]::TryParse($cols[4], [ref]$eventDuration)) { continue }
                $eventEnd = $eventStart + $eventDuration
                if ($eventStart -lt $frameEnd -and $eventEnd -gt $frameStart) {
                    $writer.WriteLine($line)
                }
            }
        } finally {
            $writer.Dispose()
            $reader.Dispose()
        }

        $frameTop = Import-Csv -Delimiter "`t" -Path $frameEventsPath |
            Where-Object { $_.exec_time_ns -match '^\d+$' } |
            Sort-Object { [Int64]$_.exec_time_ns } -Descending |
            Select-Object -First $Top
        $frameTop | Export-Csv -Delimiter "`t" -NoTypeInformation -Path $frameTopPath

        if (!$KeepRawEvents) {
            Remove-Item -LiteralPath $rawEventsPath -Force
            Remove-Item -LiteralPath $updatesPath -Force
        }

        $frameInfo = [PSCustomObject]@{
            Label = $frameLabel
            Index = if ($FrameIndex -gt 0) { $FrameIndex } else { $null }
            StartNs = $frameStart
            EndNs = $frameEnd
            DurationNs = [Int64]$selectedUpdate.exec_time_ns
            EventsPath = $frameEventsPath
            TopPath = $frameTopPath
        }
    }
}

$summary = [System.IO.StreamWriter]::new($summaryPath, $false, [System.Text.UTF8Encoding]::new($false))
try {
    $summary.WriteLine("# Tracy Trace Summary")
    $summary.WriteLine("")
    $summary.WriteLine("- Trace: ``$TracePath``")
    $summary.WriteLine("- Output: ``$OutputDir``")
    $summary.WriteLine("- Top rows: $Top")

    Write-Section $summary "Top By Max Duration"
    $summary.WriteLine("| Max ms | Mean ms | Count | Name |")
    $summary.WriteLine("| ---: | ---: | ---: | --- |")
    foreach ($row in $topByMax) {
        $summary.WriteLine("| $(Format-Ms ([Int64]$row.max_ns)) | $(Format-Ms ([Int64]$row.mean_ns)) | $($row.counts) | ``$($row.name)`` |")
    }

    Write-Section $summary "Top By Total Duration"
    $summary.WriteLine("| Total ms | Mean ms | Count | Name |")
    $summary.WriteLine("| ---: | ---: | ---: | --- |")
    foreach ($row in $topByTotal) {
        $summary.WriteLine("| $(Format-Ms ([Int64]$row.total_ns)) | $(Format-Ms ([Int64]$row.mean_ns)) | $($row.counts) | ``$($row.name)`` |")
    }

    if ($null -ne $frameInfo) {
        $title = if ($null -ne $frameInfo.Index) { "Update Frame $($frameInfo.Index)" } else { "Slowest Update Frame" }
        Write-Section $summary $title
        $summary.WriteLine("- Label: $($frameInfo.Label)")
        $summary.WriteLine("- Start ns: $($frameInfo.StartNs)")
        $summary.WriteLine("- End ns: $($frameInfo.EndNs)")
        $summary.WriteLine("- Duration ms: $(Format-Ms $frameInfo.DurationNs)")
        $summary.WriteLine("- Events: ``$($frameInfo.EventsPath)``")
        $summary.WriteLine("- Top events: ``$($frameInfo.TopPath)``")
    }

    Write-Section $summary "Generated Files"
    $summary.WriteLine("- ``stats.tsv``: all aggregate CPU zone statistics")
    $summary.WriteLine("- ``top_by_max.tsv``: zones sorted by worst single occurrence")
    $summary.WriteLine("- ``top_by_total.tsv``: zones sorted by cumulative time")
    $summary.WriteLine("- ``messages.tsv``: Tracy messages/log messages")
    $summary.WriteLine("- ``gpu.tsv``: GPU zones, if the trace contains GPU zones")
    if ($SlowFrame -or $FrameIndex -gt 0) {
        if ($KeepRawEvents) {
            $summary.WriteLine("- ``updates.tsv``: all update zones used to locate the slowest frame")
        }
        $summary.WriteLine("- ``$($frameInfo.Label)_events.tsv``: CPU zones overlapping the selected update frame")
        $summary.WriteLine("- ``$($frameInfo.Label)_top.tsv``: selected-frame zones sorted by duration")
    }
} finally {
    $summary.Dispose()
}

Write-Host "Done."
Write-Host "Summary: $summaryPath"
