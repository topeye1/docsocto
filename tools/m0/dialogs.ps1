# Find dialog windows owned by the given process names and press their safe button.
# Every click is logged (window title, buttons seen, button pressed). Used while Word/Hangul
# run unattended so a hidden confirmation box does not stall a job.
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$script:DlgOk = '^(확인|OK|예|Yes|모두 허용|허용|Allow|Allow all|닫기|Close|계속|Continue|읽기 전용|Read-Only|아니요|No)$'
function Close-Dialogs([string[]]$Names, [string]$Log) {
  $ids = @(Get-Process $Names -ErrorAction SilentlyContinue).Id
  if (-not $ids) { return 0 }
  $root = [Windows.Automation.AutomationElement]::RootElement
  $n = 0
  foreach ($w in $root.FindAll([Windows.Automation.TreeScope]::Children, [Windows.Automation.Condition]::TrueCondition)) {
    try { $pid_ = $w.Current.ProcessId; $cls = $w.Current.ClassName; $title = $w.Current.Name } catch { continue }
    if ($ids -notcontains $pid_) { continue }
    if ($cls -notmatch '#32770|NUIDialog|bosa_sdm|MsoSplash|HwpDialog|Dialog') { continue }
    $btns = $w.FindAll([Windows.Automation.TreeScope]::Descendants,
      (New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Button)))
    $names = @($btns | % { $_.Current.Name })
    # prefer the first safe button in the order of $DlgOk (no to "save changes" style questions comes last)
    $pick = $null
    foreach ($want in '확인','OK','모두 허용','허용','Allow all','Allow','계속','Continue','읽기 전용','Read-Only','예','Yes','닫기','Close','아니요','No') {
      $pick = $btns | Where-Object { $_.Current.Name -eq $want } | Select-Object -First 1; if ($pick) { break }
    }
    $msg = "$(Get-Date -Format s) dialog pid=$pid_ class=$cls title='$title' buttons=[$($names -join ', ')]"
    if ($pick) {
      try { $pick.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke(); $msg += " pressed='$($pick.Current.Name)'"; $n++ } catch { $msg += " press failed: $($_.Exception.Message)" }
    } else { $msg += ' no safe button' }
    if ($Log) { $msg | Out-File $Log -Append -Encoding utf8 }
  }
  $n
}
