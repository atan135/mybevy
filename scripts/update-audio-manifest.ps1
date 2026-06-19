param(
    [string]$AssetsAudioRoot,
    [string]$OutputPath
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptRoot "..")

if (-not $AssetsAudioRoot) {
    $AssetsAudioRoot = Join-Path $repoRoot "project\assets\audio"
}
if (-not $OutputPath) {
    $OutputPath = Join-Path $AssetsAudioRoot "audio_manifest.ron"
}

$AssetsAudioRoot = (Resolve-Path $AssetsAudioRoot).Path
$assetsRoot = (Resolve-Path (Join-Path $AssetsAudioRoot "..")).Path

function Read-AsciiString {
    param(
        [Parameter(Mandatory = $true)]
        [System.IO.BinaryReader]$Reader,
        [Parameter(Mandatory = $true)]
        [int]$Length
    )

    [System.Text.Encoding]::ASCII.GetString($Reader.ReadBytes($Length))
}

function Get-WavDurationSeconds {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $stream = [System.IO.File]::Open($Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
    try {
        $reader = [System.IO.BinaryReader]::new($stream)
        try {
            if ($stream.Length -lt 12) {
                throw "WAV file is too short."
            }

            $riffId = Read-AsciiString -Reader $reader -Length 4
            [void]$reader.ReadUInt32()
            $waveId = Read-AsciiString -Reader $reader -Length 4
            if ($riffId -ne "RIFF" -or $waveId -ne "WAVE") {
                throw "Expected RIFF/WAVE header."
            }

            $byteRate = $null
            [uint64]$dataBytes = 0

            while ($stream.Position + 8 -le $stream.Length) {
                $chunkId = Read-AsciiString -Reader $reader -Length 4
                [uint32]$chunkSize = $reader.ReadUInt32()
                $chunkStart = $stream.Position

                if ($chunkStart + $chunkSize -gt $stream.Length) {
                    throw "Chunk '$chunkId' exceeds file length."
                }

                switch ($chunkId) {
                    "fmt " {
                        if ($chunkSize -lt 16) {
                            throw "fmt chunk is too short."
                        }
                        [void]$reader.ReadUInt16()
                        [void]$reader.ReadUInt16()
                        [void]$reader.ReadUInt32()
                        $byteRate = [uint32]$reader.ReadUInt32()
                        [void]$reader.ReadUInt16()
                        [void]$reader.ReadUInt16()
                    }
                    "data" {
                        $dataBytes += [uint64]$chunkSize
                    }
                }

                $stream.Position = $chunkStart + $chunkSize
                if (($chunkSize % 2) -eq 1 -and $stream.Position -lt $stream.Length) {
                    $stream.Position += 1
                }
            }

            if ($null -eq $byteRate -or $byteRate -eq 0) {
                throw "Missing or invalid fmt byte rate."
            }
            if ($dataBytes -eq 0) {
                throw "Missing data chunk."
            }

            [double]$dataBytes / [double]$byteRate
        }
        finally {
            $reader.Dispose()
        }
    }
    finally {
        $stream.Dispose()
    }
}

function Get-ClipIdFromAssetPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$AssetPath
    )

    $withoutPrefix = $AssetPath -replace '^audio/', ''
    $withoutExtension = $withoutPrefix -replace '\.[^./]+$', ''
    $withoutExtension -replace '/', '.'
}

function ConvertTo-RonString {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value
    )

    '"' + ($Value -replace '\\', '\\' -replace '"', '\"') + '"'
}

$audioExtensions = @{
    ".wav" = "wav"
    ".ogg" = "unsupported"
    ".mp3" = "unsupported"
    ".flac" = "unsupported"
}

$clips = @()
$errors = @()

Get-ChildItem -Path $AssetsAudioRoot -Recurse -File |
    Where-Object { $_.FullName -ne (Resolve-Path -LiteralPath $OutputPath -ErrorAction SilentlyContinue).Path } |
    Sort-Object FullName |
    ForEach-Object {
        $file = $_
        $extension = $file.Extension.ToLowerInvariant()
        if (-not $audioExtensions.ContainsKey($extension)) {
            return
        }

        $relativeToAssets = [System.IO.Path]::GetRelativePath($assetsRoot, $file.FullName).Replace('\', '/')
        $clipId = Get-ClipIdFromAssetPath -AssetPath $relativeToAssets

        try {
            switch ($audioExtensions[$extension]) {
                "wav" {
                    $duration = Get-WavDurationSeconds -Path $file.FullName
                    $clips += [pscustomobject]@{
                        Id = $clipId
                        Path = $relativeToAssets
                        DurationSeconds = $duration
                    }
                }
                default {
                    $errors += "Unsupported audio format '$extension' for $relativeToAssets. Add a parser before including it in audio_manifest.ron."
                }
            }
        }
        catch {
            $errors += "Failed to parse $relativeToAssets`: $($_.Exception.Message)"
        }
    }

if ($errors.Count -gt 0) {
    throw ($errors -join [Environment]::NewLine)
}

$lines = [System.Collections.Generic.List[string]]::new()
$lines.Add("(")
$lines.Add("    clips: [")
foreach ($clip in $clips) {
    $duration = $clip.DurationSeconds.ToString("0.########", [System.Globalization.CultureInfo]::InvariantCulture)
    $lines.Add(("        (id: {0}, path: {1}, duration_seconds: {2})," -f (ConvertTo-RonString $clip.Id), (ConvertTo-RonString $clip.Path), $duration))
}
$lines.Add("    ],")
$lines.Add(")")

Set-Content -Path $OutputPath -Value $lines -Encoding UTF8
Write-Host "Wrote $($clips.Count) audio clip durations to $OutputPath"
