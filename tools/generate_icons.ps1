Add-Type -AssemblyName System.Drawing

$src = Join-Path $PSScriptRoot '..\src\assets\icono.png'
$outDir = Join-Path $PSScriptRoot '..\src-tauri\icons'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$source = [System.Drawing.Image]::FromFile($src)
$side = [Math]::Min($source.Width, $source.Height)
$cropX = [int](($source.Width - $side) / 2)
$cropY = [int](($source.Height - $side) / 2)
$sourceRect = New-Object System.Drawing.Rectangle($cropX, $cropY, $side, $side)

function Render-Logo([int]$size, [System.Drawing.Rectangle]$srcRect, [System.Drawing.Image]$srcImg) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.DrawImage($srcImg, (New-Object System.Drawing.Rectangle(0, 0, $size, $size)), $srcRect, [System.Drawing.GraphicsUnit]::Pixel)
    $g.Dispose()
    return $bmp
}
$pngSizes = @(
    @{ Size = 32;  File = '32x32.png' },
    @{ Size = 128; File = '128x128.png' },
    @{ Size = 256; File = '128x128@2x.png' },
    @{ Size = 512; File = 'icon.png' }
)
foreach ($p in $pngSizes) {
    $bmp = Render-Logo $p.Size $sourceRect $source
    $bmp.Save((Join-Path $outDir $p.File), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}
$sizes = @(16, 20, 24, 28, 32, 36, 40, 48, 56, 64, 72, 80, 96, 112, 128, 160, 192, 256)
$frames = @()
foreach ($s in $sizes) {
    $bmp = Render-Logo $s $sourceRect $source
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $frames += ,@{ Size = $s; Bytes = $ms.ToArray() }
    $ms.Dispose()
    $bmp.Dispose()
}
$ico = Join-Path $outDir 'icon.ico'
$fs = [System.IO.File]::Create($ico)
$bw = New-Object System.IO.BinaryWriter($fs)
$bw.Write([uint16]0)
$bw.Write([uint16]1)
$bw.Write([uint16]$frames.Count)
$offset = 6 + (16 * $frames.Count)
foreach ($f in $frames) {
    $s = $f.Size
    $bytes = $f.Bytes
    $bw.Write([byte]($(if ($s -ge 256) {0} else {$s})))
    $bw.Write([byte]($(if ($s -ge 256) {0} else {$s})))
    $bw.Write([byte]0)
    $bw.Write([byte]0)
    $bw.Write([uint16]1)
    $bw.Write([uint16]32)
    $bw.Write([uint32]$bytes.Length)
    $bw.Write([uint32]$offset)
    $offset += $bytes.Length
}
foreach ($f in $frames) {
    $bw.Write($f.Bytes)
}
$bw.Dispose()
$fs.Dispose()

$source.Dispose()
Write-Output 'Iconos generados a partir de icono.png:'
Get-ChildItem $outDir | Select-Object Name, Length | Format-Table -AutoSize