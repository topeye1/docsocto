# Find dialog windows owned by the given process names and press their safe button.
# Every click is logged (window title, buttons seen, button pressed). Used while Word/Hangul
# run unattended so a hidden confirmation box does not stall a job.
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
if (-not ('DlgU32' -as [type])) {
  Add-Type 'using System; using System.Runtime.InteropServices; public class DlgU32 { [DllImport("user32")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l); }'
}
$script:DlgOk = '^(확인|OK|예|Yes|모두 허용|허용|Allow|Allow all|닫기|Close|계속|Continue|읽기 전용|Read-Only|아니요|No)$'
function Close-Dialogs([string[]]$Names, [string]$Log) {
  $ids = @(Get-Process $Names -ErrorAction SilentlyContinue).Id
  if (-not $ids) { return 0 }
  $root = [Windows.Automation.AutomationElement]::RootElement
  $n = 0
  foreach ($w in $root.FindAll([Windows.Automation.TreeScope]::Children, [Windows.Automation.Condition]::TrueCondition)) {
    try { $pid_ = $w.Current.ProcessId; $cls = $w.Current.ClassName; $title = $w.Current.Name } catch { continue }
    if ($ids -notcontains $pid_) { continue }
    if ($cls -notmatch '#32770|NUIDialog|bosa_sdm|MsoSplash|HwpDialog|Dialog|MessageBoxImpl') { continue }
    $btns = $w.FindAll([Windows.Automation.TreeScope]::Descendants,
      (New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Button)))
    $names = @($btns | % { $_.Current.Name })
    # prefer the first safe button in the order of $DlgOk (no to "save changes" style questions comes last)
    $pick = $null
    foreach ($want in '모두 허용','접근 허용','확인','OK','허용','Allow all','Allow','계속','Continue','읽기 전용','Read-Only','예','Yes','닫기','Close','아니요','No') {
      # 한글 단추는 끝에 단축키 글자가 붙는다(예: '모두 허용N', '확인(&O)')
      $pick = $btns | Where-Object { ($_.Current.Name -replace '\(&?\w\)$', '' -replace '(?<=\S)[A-Z]$', '').Trim() -eq $want } | Select-Object -First 1; if ($pick) { break }
    }
    $msg = "$(Get-Date -Format s) dialog pid=$pid_ class=$cls title='$title' buttons=[$($names -join ', ')]"
    if ($pick) {
      # 한글 MessageBoxImpl 단추는 창 핸들이 없어 Invoke가 먹지 않는다 — 단축키 글자를 창에 보낸다.
      $acc = [regex]::Match($pick.Current.Name, '\(&?(\w)\)$|(?<=\S)([A-Z])$')
      if ($pick.Current.NativeWindowHandle -eq 0 -and $acc.Success) {
        $vk = [IntPtr][int][char]("$($acc.Groups[1].Value)$($acc.Groups[2].Value)".ToUpper())
        $hw = [IntPtr]$w.Current.NativeWindowHandle
        [void][DlgU32]::PostMessage($hw, 0x0100, $vk, [IntPtr]::Zero); [void][DlgU32]::PostMessage($hw, 0x0101, $vk, [IntPtr]::Zero)
        $msg += " pressed='$($pick.Current.Name)' (key)"; $n++
      } else {
        try { $pick.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke(); $msg += " pressed='$($pick.Current.Name)'"; $n++ } catch { $msg += " press failed: $($_.Exception.Message)" }
      }
    } else { $msg += ' no safe button' }
    if ($Log) { $msg | Out-File $Log -Append -Encoding utf8 }
  }
  $n
}
