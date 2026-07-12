param(
    [string]$OutputDirectory = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

Add-Type -AssemblyName System.Drawing

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $PSScriptRoot "../project/assets/ui/icons"
}
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
[System.IO.Directory]::CreateDirectory($OutputDirectory) | Out-Null

$script:CanvasSize = 96
$script:Scale = 4.0

function New-IconBitmap {
    $bitmap = [System.Drawing.Bitmap]::new(
        $script:CanvasSize,
        $script:CanvasSize,
        [System.Drawing.Imaging.PixelFormat]::Format32bppArgb
    )
    $bitmap.SetResolution(96.0, 96.0)
    return $bitmap
}

function New-RoundPen {
    param(
        [System.Drawing.Color]$Color,
        [float]$Width = 2.0
    )

    $pen = [System.Drawing.Pen]::new($Color, $Width * $script:Scale)
    $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
    return $pen
}

function Save-IconBitmap {
    param(
        [System.Drawing.Bitmap]$Bitmap,
        [string]$Name
    )

    $path = Join-Path $OutputDirectory "$Name.png"
    $Bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $Bitmap.Dispose()
}

function New-MonochromeIcon {
    param(
        [string]$Name,
        [scriptblock]$Draw
    )

    $bitmap = New-IconBitmap
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.Clear([System.Drawing.Color]::Transparent)
        $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
        $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        & $Draw $graphics
    }
    finally {
        $graphics.Dispose()
    }
    Save-IconBitmap -Bitmap $bitmap -Name $Name
}

# Geometry follows Lucide 0.468.0's 24 px icons and is rasterized at 4x.
# Upstream source URLs and the ISC license are pinned in assets/ui/icons/manifest.ron.
New-MonochromeIcon "add" {
    param($graphics)
    $pen = New-RoundPen ([System.Drawing.Color]::White)
    try {
        $graphics.DrawLine($pen, 5 * $script:Scale, 12 * $script:Scale, 19 * $script:Scale, 12 * $script:Scale)
        $graphics.DrawLine($pen, 12 * $script:Scale, 5 * $script:Scale, 12 * $script:Scale, 19 * $script:Scale)
    }
    finally {
        $pen.Dispose()
    }
}

New-MonochromeIcon "remove" {
    param($graphics)
    $pen = New-RoundPen ([System.Drawing.Color]::White)
    try {
        $graphics.DrawLine($pen, 5 * $script:Scale, 12 * $script:Scale, 19 * $script:Scale, 12 * $script:Scale)
    }
    finally {
        $pen.Dispose()
    }
}

New-MonochromeIcon "help" {
    param($graphics)
    $pen = New-RoundPen ([System.Drawing.Color]::White)
    $dot = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::White)
    try {
        $graphics.DrawEllipse($pen, 2 * $script:Scale, 2 * $script:Scale, 20 * $script:Scale, 20 * $script:Scale)
        $graphics.DrawArc($pen, 8.4 * $script:Scale, 6.4 * $script:Scale, 7.2 * $script:Scale, 7.2 * $script:Scale, 190, 235)
        $graphics.DrawLine($pen, 12 * $script:Scale, 13.1 * $script:Scale, 12 * $script:Scale, 14.1 * $script:Scale)
        $graphics.FillEllipse($dot, 11.1 * $script:Scale, 16.8 * $script:Scale, 1.8 * $script:Scale, 1.8 * $script:Scale)
    }
    finally {
        $dot.Dispose()
        $pen.Dispose()
    }
}

New-MonochromeIcon "close" {
    param($graphics)
    $pen = New-RoundPen ([System.Drawing.Color]::White)
    try {
        $graphics.DrawLine($pen, 18 * $script:Scale, 6 * $script:Scale, 6 * $script:Scale, 18 * $script:Scale)
        $graphics.DrawLine($pen, 6 * $script:Scale, 6 * $script:Scale, 18 * $script:Scale, 18 * $script:Scale)
    }
    finally {
        $pen.Dispose()
    }
}

New-MonochromeIcon "chevron-down" {
    param($graphics)
    $pen = New-RoundPen ([System.Drawing.Color]::White)
    try {
        $graphics.DrawLine($pen, 6 * $script:Scale, 9 * $script:Scale, 12 * $script:Scale, 15 * $script:Scale)
        $graphics.DrawLine($pen, 12 * $script:Scale, 15 * $script:Scale, 18 * $script:Scale, 9 * $script:Scale)
    }
    finally {
        $pen.Dispose()
    }
}

New-MonochromeIcon "loading" {
    param($graphics)
    for ($index = 0; $index -lt 8; $index++) {
        $angle = ((-90 + ($index * 45)) * [Math]::PI) / 180.0
        $alpha = 80 + ($index * 25)
        $color = [System.Drawing.Color]::FromArgb([Math]::Min(255, $alpha), 255, 255, 255)
        $pen = New-RoundPen $color 1.8
        try {
            $innerX = (12 + ([Math]::Cos($angle) * 6.0)) * $script:Scale
            $innerY = (12 + ([Math]::Sin($angle) * 6.0)) * $script:Scale
            $outerX = (12 + ([Math]::Cos($angle) * 9.0)) * $script:Scale
            $outerY = (12 + ([Math]::Sin($angle) * 9.0)) * $script:Scale
            $graphics.DrawLine($pen, $innerX, $innerY, $outerX, $outerY)
        }
        finally {
            $pen.Dispose()
        }
    }
}

New-MonochromeIcon "arrow-left" {
    param($graphics)
    $pen = New-RoundPen ([System.Drawing.Color]::White)
    try {
        $graphics.DrawLine($pen, 19 * $script:Scale, 12 * $script:Scale, 5 * $script:Scale, 12 * $script:Scale)
        $graphics.DrawLine($pen, 12 * $script:Scale, 5 * $script:Scale, 5 * $script:Scale, 12 * $script:Scale)
        $graphics.DrawLine($pen, 5 * $script:Scale, 12 * $script:Scale, 12 * $script:Scale, 19 * $script:Scale)
    }
    finally {
        $pen.Dispose()
    }
}

New-MonochromeIcon "arrow-right" {
    param($graphics)
    $pen = New-RoundPen ([System.Drawing.Color]::White)
    try {
        $graphics.DrawLine($pen, 5 * $script:Scale, 12 * $script:Scale, 19 * $script:Scale, 12 * $script:Scale)
        $graphics.DrawLine($pen, 12 * $script:Scale, 5 * $script:Scale, 19 * $script:Scale, 12 * $script:Scale)
        $graphics.DrawLine($pen, 19 * $script:Scale, 12 * $script:Scale, 12 * $script:Scale, 19 * $script:Scale)
    }
    finally {
        $pen.Dispose()
    }
}

$badge = New-IconBitmap
$badgeGraphics = [System.Drawing.Graphics]::FromImage($badge)
try {
    $badgeGraphics.Clear([System.Drawing.Color]::Transparent)
    $badgeGraphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $badgePath = [System.Drawing.Drawing2D.GraphicsPath]::new()
    $badgePath.AddPolygon([System.Drawing.PointF[]]@(
        [System.Drawing.PointF]::new(48, 6),
        [System.Drawing.PointF]::new(88, 48),
        [System.Drawing.PointF]::new(48, 90),
        [System.Drawing.PointF]::new(8, 48)
    ))
    $badgeFill = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
        [System.Drawing.PointF]::new(12, 12),
        [System.Drawing.PointF]::new(84, 84),
        [System.Drawing.Color]::FromArgb(255, 52, 211, 153),
        [System.Drawing.Color]::FromArgb(255, 45, 124, 224)
    )
    $badgePen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(255, 235, 244, 255), 5)
    $badgePen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
    $core = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(255, 255, 211, 77))
    try {
        $badgeGraphics.FillPath($badgeFill, $badgePath)
        $badgeGraphics.DrawPath($badgePen, $badgePath)
        $badgeGraphics.FillEllipse($core, 35, 35, 26, 26)
    }
    finally {
        $core.Dispose()
        $badgePen.Dispose()
        $badgeFill.Dispose()
        $badgePath.Dispose()
    }
}
finally {
    $badgeGraphics.Dispose()
}
Save-IconBitmap -Bitmap $badge -Name "full-color-badge"

$missing = New-IconBitmap
$missingGraphics = [System.Drawing.Graphics]::FromImage($missing)
try {
    $missingGraphics.Clear([System.Drawing.Color]::Transparent)
    $missingGraphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $fill = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(255, 226, 64, 138))
    $border = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(255, 255, 226, 242), 6)
    $slash = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(255, 30, 18, 27), 8)
    $slash.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $slash.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    try {
        $missingGraphics.FillRectangle($fill, 13, 13, 70, 70)
        $missingGraphics.DrawRectangle($border, 13, 13, 70, 70)
        $missingGraphics.DrawLine($slash, 27, 69, 69, 27)
    }
    finally {
        $slash.Dispose()
        $border.Dispose()
        $fill.Dispose()
    }
}
finally {
    $missingGraphics.Dispose()
}
Save-IconBitmap -Bitmap $missing -Name "missing"

Get-ChildItem -LiteralPath $OutputDirectory -Filter "*.png" |
    Sort-Object Name |
    ForEach-Object {
        $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        Write-Output "$($_.Name) $hash"
    }
