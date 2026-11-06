# Paper Pilot - controle met UI Automation (zoals een schermlezer het programma ziet).
# Hard: elk onderdeel dat focus kan krijgen heeft een naam, en er is een menubalk.
# Zacht (waarschuwing): Tab, F1 en Escape werken met het toetsenbord.
param([string]$Exe = "target\release\PaperPilot.exe")
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms
$A = [System.Windows.Automation.AutomationElement]
$fouten = 0
$p = Start-Process $Exe -ArgumentList "--demo", "--taal", "nl" -PassThru
try {
    $venster = $null
    for ($i = 0; $i -lt 40 -and -not $venster; $i++) {
        Start-Sleep -Milliseconds 250
        $c = New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, $p.Id)
        $venster = $A::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $c)
    }
    if (-not $venster) { throw "Het venster van Paper Pilot is niet gevonden." }
    "Venster: $($venster.Current.Name) (klasse $($venster.Current.ClassName), aan: $($venster.Current.IsEnabled))"
    Start-Sleep -Seconds 2
    # Wat de schermlezer ziet: de boom tot drie lagen diep.
    $loper = [System.Windows.Automation.TreeWalker]::RawViewWalker
    function Boom($e, $diepte) {
        if ($diepte -gt 3) { return }
        $k = $loper.GetFirstChild($e)
        while ($k) {
            "$('  ' * $diepte)- $($k.Current.ControlType.ProgrammaticName) | $($k.Current.Name) | $($k.Current.ClassName) | focus: $($k.Current.IsKeyboardFocusable)"
            Boom $k ($diepte + 1)
            $k = $loper.GetNextSibling($k)
        }
    }
    Boom $venster 1
    "Programma reageert: $((Get-Process -Id $p.Id).Responding)"
    # Tweede manier: MSAA (IAccessible), zoals JAWS en NVDA het ook opvragen.
    Add-Type -AssemblyName Accessibility
    Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public static class Msaa {
  [DllImport("oleacc.dll")] public static extern int AccessibleObjectFromWindow(IntPtr hwnd, uint id, ref Guid iid, [MarshalAs(UnmanagedType.IUnknown)] out object acc);
  [DllImport("user32.dll")] public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string klasse, string titel);
}
"@
    $iid = [Guid]"618736E0-3C3D-11CF-810C-00AA00389B71"
    $hwnd = [IntPtr]$venster.Current.NativeWindowHandle
    foreach ($soort in @(@("venster", [uint32]0xFFFFFFFC), @("menubalk", [uint32]0xFFFFFFFD))) {
        $acc = $null
        $r = [Msaa]::AccessibleObjectFromWindow($hwnd, $soort[1], [ref]$iid, [ref]$acc)
        if ($r -ne 0 -or -not $acc) { "MSAA $($soort[0]): niet gelukt ($r)"; continue }
        $acc = [Accessibility.IAccessible]$acc
        "MSAA $($soort[0]): rol $($acc.accRole(0)), kinderen $($acc.accChildCount)"
        for ($i = 1; $i -le $acc.accChildCount; $i++) {
            try { "  - rol $($acc.accRole($i)) | $($acc.accName($i))" } catch { "  - kind ${i}: $_" }
        }
    }
    $knop = [Msaa]::FindWindowEx($hwnd, [IntPtr]::Zero, "Button", $null)
    if ($knop -ne [IntPtr]::Zero) {
        $acc = $null
        $r = [Msaa]::AccessibleObjectFromWindow($knop, [uint32]0xFFFFFFFC, [ref]$iid, [ref]$acc)
        if ($r -eq 0 -and $acc) { $acc = [Accessibility.IAccessible]$acc; "MSAA eerste knop: rol $($acc.accRole(0)) (43 = knop), status $($acc.accState(0)), naam $($acc.accName(0))" }
        else { "MSAA eerste knop: niet gelukt ($r)" }
    }
    $alles = $venster.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    "Onderdelen gevonden: $($alles.Count)"
    $menubalk = 0
    foreach ($e in $alles) {
        $soort = $e.Current.ControlType.ProgrammaticName
        if ($soort -eq "ControlType.MenuBar") { $menubalk++ }
        if ($e.Current.IsKeyboardFocusable) {
            if ([string]::IsNullOrWhiteSpace($e.Current.Name)) {
                "FOUT: $soort zonder naam"; $fouten++
            } else {
                "Met focus: $soort - $($e.Current.Name)"
            }
        }
    }
    if ($menubalk -lt 1) { "FOUT: geen menubalk gevonden"; $fouten++ }

    # Toetsenbord (alleen waarschuwingen: focus op een bouwcomputer is niet altijd betrouwbaar).
    try {
        $venster.SetFocus()
        Start-Sleep -Milliseconds 300
        $voor = $A::FocusedElement.Current.Name
        [System.Windows.Forms.SendKeys]::SendWait("{TAB}")
        Start-Sleep -Milliseconds 300
        $na = $A::FocusedElement.Current.Name
        if ($voor -eq $na) { Write-Warning "Tab verplaatste de focus niet ($voor)." } else { "Tab: van '$voor' naar '$na'" }
        [System.Windows.Forms.SendKeys]::SendWait("{F1}")
        Start-Sleep -Milliseconds 800
        $f = $A::FocusedElement
        "Na F1 staat de focus op: $($f.Current.ControlType.ProgrammaticName) - $($f.Current.Name)"
        [System.Windows.Forms.SendKeys]::SendWait("{ESC}")
        Start-Sleep -Milliseconds 500
        "Na Escape staat de focus op: $($A::FocusedElement.Current.Name)"
    } catch { Write-Warning "Toetsenbordproef: $_" }
} finally {
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
}
if ($fouten -gt 0) { throw "$fouten fout(en) bij de toegankelijkheid." }
"GESLAAGD"
