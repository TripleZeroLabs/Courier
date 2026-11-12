# Generates Courier's original artwork with System.Drawing:
#   tools/icon-source.png           1024px app icon (feed to `npx tauri icon`)
#   src-tauri/icons/tray.png        32px tray icon
#   src-tauri/icons/tray-unread.png 32px tray icon with an unread dot
#   src-tauri/icons/badge-1..9.png, badge-dot.png  taskbar overlay badges (32px)
#
# Run from the repo root:  powershell -ExecutionPolicy Bypass -File tools\make-icons.ps1
# Then:                    npx tauri icon tools\icon-source.png

Add-Type -AssemblyName System.Drawing
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$icons = Join-Path $root 'src-tauri\icons'
New-Item -ItemType Directory -Force $icons | Out-Null

function New-RoundRect([single]$x, [single]$y, [single]$w, [single]$h, [single]$r) {
    $p = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = 2 * $r
    $p.AddArc($x, $y, $d, $d, 180, 90)
    $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
    $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
    $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
    $p.CloseFigure()
    return $p
}

# Draws the Courier mark at size $s: a round chat bubble with a tail at the
# bottom-left, filled with a light-to-deep blue gradient, and three white
# "typing" dots. (Deliberately not Messenger's lightning bolt, which is
# Meta's trademark.) $tile is accepted for compatibility and ignored.
function Draw-Mark([System.Drawing.Graphics]$g, [int]$s, [bool]$tile) {
    $g.SmoothingMode = 'AntiAlias'
    $g.PixelOffsetMode = 'HighQuality'
    $light = [System.Drawing.Color]::FromArgb(255, 0, 178, 255)
    $deep  = [System.Drawing.Color]::FromArgb(255, 0, 92, 230)

    # Bubble: a circle plus a tail, as one path so the gradient is continuous.
    $cx = $s * 0.5; $cy = $s * 0.47; $rad = $s * 0.45
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $path.FillMode = 'Winding'
    $path.AddEllipse([single]($cx - $rad), [single]($cy - $rad), [single](2 * $rad), [single](2 * $rad))
    $tail = @(
        (New-Object System.Drawing.PointF ($s * 0.15), ($s * 0.66)),
        (New-Object System.Drawing.PointF ($s * 0.40), ($s * 0.88)),
        (New-Object System.Drawing.PointF ($s * 0.10), ($s * 0.98))
    )
    $path.AddPolygon([System.Drawing.PointF[]]$tail)

    $rect = New-Object System.Drawing.RectangleF 0, 0, $s, $s
    $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, $light, $deep, 70
    $g.FillPath($brush, $path)

    # Three dots.
    $r = $s * 0.065
    foreach ($i in -1, 0, 1) {
        $dx = $cx + $i * $s * 0.20
        $g.FillEllipse([System.Drawing.Brushes]::White, [single]($dx - $r), [single]($cy - $r), [single](2 * $r), [single](2 * $r))
    }
}

function Save-Png([int]$s, [string]$path, [scriptblock]$draw) {
    $bmp = New-Object System.Drawing.Bitmap $s, $s, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.TextRenderingHint = 'AntiAliasGridFit'
    & $draw $g $s
    $g.Dispose()
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Host "wrote $path"
}

$red = [System.Drawing.Color]::FromArgb(255, 229, 57, 53)

Save-Png 1024 (Join-Path $PSScriptRoot 'icon-source.png') { param($g, $s) Draw-Mark $g $s $true }
Save-Png 32 (Join-Path $icons 'tray.png') { param($g, $s) Draw-Mark $g $s $true }
Save-Png 32 (Join-Path $icons 'tray-unread.png') {
    param($g, $s)
    Draw-Mark $g $s $true
    $b = New-Object System.Drawing.SolidBrush $red
    $g.FillEllipse([System.Drawing.Brushes]::White, 17, 0, 15, 15)
    $g.FillEllipse($b, 18.5, 1.5, 12, 12)
}

# Taskbar overlay badges: red disc, white number (Windows shows these at 16px).
function Draw-Badge($g, $s, [string]$text) {
    $g.SmoothingMode = 'AntiAlias'
    $b = New-Object System.Drawing.SolidBrush $red
    $g.FillEllipse([System.Drawing.Brushes]::White, 0, 0, $s, $s)
    $g.FillEllipse($b, 1.5, 1.5, $s - 3, $s - 3)
    if ($text) {
        $font = New-Object System.Drawing.Font 'Segoe UI', ($s * 0.56), ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
        $fmt = New-Object System.Drawing.StringFormat
        $fmt.Alignment = 'Center'; $fmt.LineAlignment = 'Center'
        $rect = New-Object System.Drawing.RectangleF 0, 1, $s, $s
        $g.DrawString($text, $font, [System.Drawing.Brushes]::White, $rect, $fmt)
    }
}
foreach ($n in 1..9) {
    Save-Png 32 (Join-Path $icons "badge-$n.png") ([scriptblock]::Create("param(`$g, `$s) Draw-Badge `$g `$s '$n'"))
}
Save-Png 32 (Join-Path $icons 'badge-dot.png') {
    param($g, $s)
    $g.SmoothingMode = 'AntiAlias'
    $b = New-Object System.Drawing.SolidBrush $red
    $g.FillEllipse([System.Drawing.Brushes]::White, 4, 4, 24, 24)
    $g.FillEllipse($b, 6, 6, 20, 20)
}
