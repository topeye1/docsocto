# Word: one docx -> PDF; prints page count. Run in its own process so a hang can be killed.
param([string]$Src, [string]$Pdf)
$ErrorActionPreference = 'Stop'
$w = New-Object -ComObject Word.Application
$w.Visible = $false; $w.DisplayAlerts = 0
try {
  $doc = $w.Documents.Open($Src, $false, $true, $false)
  $n = $doc.ComputeStatistics(2)
  $doc.ExportAsFixedFormat($Pdf, 17); $doc.Close(0); "PAGES=$n"
} finally { $w.Quit() }
