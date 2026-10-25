Add-Type -AssemblyName System.Drawing
$outDir = 'C:\Users\Lenovo\.zcode\workspace\default\icons'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

function New-IconPng([int]$Size, [string]$SaveFile) {
  $bmp = New-Object System.Drawing.Bitmap($Size, $Size)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality

  $r = [int]([Math]::Max(2, $Size * 0.18))
  $d = 2 * $r
  $gp = New-Object System.Drawing.Drawing2D.GraphicsPath
  $gp.AddArc(0, 0, $d, $d, 180, 90)
  $gp.AddArc($Size - $d, 0, $d, $d, 270, 90)
  $gp.AddArc($Size - $d, $Size - $d, $d, $d, 0, 90)
  $gp.AddArc(0, $Size - $d, $d, $d, 90, 90)
  $gp.CloseFigure()
  $bgBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 0x0B, 0x6E, 0xD4))
  $g.FillPath($bgBrush, $gp)

  $fontSize = [float]([Math]::Round($Size * 0.56))
  $fnt = New-Object System.Drawing.Font('Microsoft YaHei', $fontSize,
    [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
  $fmt = New-Object System.Drawing.StringFormat
  $fmt.Alignment = [System.Drawing.StringAlignment]::Center
  $fmt.LineAlignment = [System.Drawing.StringAlignment]::Center
  $txtRect = New-Object System.Drawing.RectangleF(0, ($Size * -0.02), $Size, $Size)
  $g.DrawString([string][char]0x8BD1, $fnt, [System.Drawing.Brushes]::White, $txtRect, $fmt)

  $bmp.Save($SaveFile, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose(); $fnt.Dispose(); $gp.Dispose(); $bgBrush.Dispose()
}

foreach ($s in 256, 64, 48, 32, 16) {
  New-IconPng $s (Join-Path $outDir ("icon_$s.png"))
}
Write-Output 'PNG done'
Get-ChildItem $outDir | ForEach-Object { Write-Output ($_.Name + ' ' + $_.Length + 'B') }
