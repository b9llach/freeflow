# Generates icon.svg, sized PNGs, and a multi-resolution icon.ico for Freeflow.
# Drawn with GDI+ so there is no external dependency. Re-run whenever the
# icon design changes.

param(
    [string]$OutDir = "src-tauri/icons"
)

Add-Type -AssemblyName System.Drawing

# All standard Windows shell icon sizes so the taskbar, Start menu, file
# explorer, and alt-tab have a native frame at every DPI scaling factor.
# 100%=32, 125%=40, 150%=48, 175%=56, 200%=64.
$sizes = 16, 20, 24, 32, 40, 48, 56, 64, 96, 128, 256

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$svg = @'
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <circle cx="50" cy="50" r="50" fill="#ffffff"/>
  <rect x="18"   y="38" width="13" height="24" rx="6.5" fill="#6d28d9"/>
  <rect x="35"   y="27" width="13" height="46" rx="6.5" fill="#8b5cf6"/>
  <rect x="52"   y="16" width="13" height="68" rx="6.5" fill="#6d28d9"/>
  <rect x="69"   y="33" width="13" height="34" rx="6.5" fill="#8b5cf6"/>
</svg>
'@
[System.IO.File]::WriteAllText(
    (Join-Path $OutDir "icon.svg"),
    $svg,
    [System.Text.UTF8Encoding]::new($false)
)

function Add-RoundRect {
    param($Graphics, $Brush, [single]$X, [single]$Y, [single]$W, [single]$H, [single]$R)
    $d = $R * 2.0
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $path.AddArc($X,          $Y,          $d, $d, 180, 90)
    $path.AddArc($X + $W - $d,$Y,          $d, $d, 270, 90)
    $path.AddArc($X + $W - $d,$Y + $H - $d,$d, $d,   0, 90)
    $path.AddArc($X,          $Y + $H - $d,$d, $d,  90, 90)
    $path.CloseFigure()
    $Graphics.FillPath($Brush, $path)
    $path.Dispose()
}

function New-FreeflowIcon {
    param([int]$Size)

    $bmp = New-Object System.Drawing.Bitmap(
        $Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb
    )
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode     = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode   = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)

    # White circle. Use RectangleF to disambiguate the overload and inset
    # by a sub-pixel so anti-aliased edges stay inside the bitmap.
    $white = [System.Drawing.Brushes]::White
    $rect = New-Object System.Drawing.RectangleF(
        [single]0.0, [single]0.0, [single]$Size, [single]$Size
    )
    $g.FillEllipse($white, $rect)

    $violetDark = New-Object System.Drawing.SolidBrush(
        [System.Drawing.Color]::FromArgb(109, 40, 217)   # violet-700
    )
    $violetMid = New-Object System.Drawing.SolidBrush(
        [System.Drawing.Color]::FromArgb(139, 92, 246)   # violet-500
    )

    $scale = [single]($Size / 100.0)
    $barW  = [single](13.0 * $scale)
    $rad   = [single](6.5  * $scale)

    $bars = @(
        @{ xv = 18.0; yv = 38.0; hv = 24.0; brush = $violetDark },
        @{ xv = 35.0; yv = 27.0; hv = 46.0; brush = $violetMid  },
        @{ xv = 52.0; yv = 16.0; hv = 68.0; brush = $violetDark },
        @{ xv = 69.0; yv = 33.0; hv = 34.0; brush = $violetMid  }
    )

    foreach ($b in $bars) {
        $x = [single]($b.xv * $scale)
        $y = [single]($b.yv * $scale)
        $h = [single]($b.hv * $scale)
        Add-RoundRect -Graphics $g -Brush $b.brush -X $x -Y $y -W $barW -H $h -R $rad
    }

    $violetDark.Dispose()
    $violetMid.Dispose()
    $g.Dispose()
    return $bmp
}

$pngPaths = @{}
foreach ($size in $sizes) {
    $path = Join-Path $OutDir "icon-$size.png"
    $bmp  = New-FreeflowIcon -Size $size
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    $pngPaths[$size] = $path
}

Copy-Item $pngPaths[32]  (Join-Path $OutDir "32x32.png")       -Force
Copy-Item $pngPaths[128] (Join-Path $OutDir "128x128.png")     -Force
Copy-Item $pngPaths[256] (Join-Path $OutDir "128x128@2x.png")  -Force

# Build multi-resolution icon.ico with embedded PNG data at every size.
$icoPath = Join-Path $OutDir "icon.ico"
$fs = [System.IO.File]::Create($icoPath)
$bw = New-Object System.IO.BinaryWriter($fs)

$frames = @()
foreach ($size in $sizes) {
    $frames += [pscustomobject]@{
        Size  = $size
        Bytes = [System.IO.File]::ReadAllBytes($pngPaths[$size])
    }
}

$bw.Write([uint16]0)                    # reserved
$bw.Write([uint16]1)                    # type: icon
$bw.Write([uint16]$frames.Count)        # number of images

$offset = 6 + ($frames.Count * 16)
foreach ($f in $frames) {
    $dim = if ($f.Size -ge 256) { [byte]0 } else { [byte]$f.Size }
    $bw.Write($dim)                     # width
    $bw.Write($dim)                     # height
    $bw.Write([byte]0)                  # palette count
    $bw.Write([byte]0)                  # reserved
    $bw.Write([uint16]1)                # color planes
    $bw.Write([uint16]32)               # bits per pixel
    $bw.Write([uint32]$f.Bytes.Length)
    $bw.Write([uint32]$offset)
    $offset += $f.Bytes.Length
}
foreach ($f in $frames) {
    $bw.Write($f.Bytes)
}

$bw.Close()
$fs.Close()

Write-Host "Wrote $($sizes.Length) PNG sizes, icon.ico, and icon.svg to $OutDir"
