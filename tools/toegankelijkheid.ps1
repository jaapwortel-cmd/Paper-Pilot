# Paper Pilot - controle met UI Automation (zoals een schermlezer het programma ziet).
# Hard (via MSAA, zoals JAWS en NVDA): de knoppen zijn drukknoppen met een naam en focus, de menubalk heeft drie menu's.
# UIA wordt ook getoond, maar alleen als waarschuwing: op de bouwcomputer van GitHub werken de UIA-proxy's niet.
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
    # Tweede manier: MSAA (IAccessible), zoals JAWS en NVDA het opvragen. Dit is de harde controle.
    Add-Type -ReferencedAssemblies Accessibility -TypeDefinition @"
using System; using System.Text; using System.Runtime.InteropServices; using Accessibility;
public static class Msaa {
  [DllImport("oleacc.dll")] static extern int AccessibleObjectFromWindow(IntPtr hwnd, uint id, ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out IAccessible acc);
  [DllImport("user32.dll")] public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string klasse, string titel);
  static IAccessible Haal(IntPtr hwnd, uint id) {
    Guid iid = new Guid("618736E0-3C3D-11CF-810C-00AA00389B71"); IAccessible acc;
    return AccessibleObjectFromWindow(hwnd, id, ref iid, out acc) == 0 ? acc : null;
  }
  public static int Rol(IntPtr hwnd, uint id) { var a = Haal(hwnd, id); return a == null ? -1 : Convert.ToInt32(a.get_accRole(0)); }
  public static string Naam(IntPtr hwnd, uint id) { var a = Haal(hwnd, id); return a == null ? null : a.get_accName(0); }
  public static int Status(IntPtr hwnd, uint id) { var a = Haal(hwnd, id); return a == null ? -1 : Convert.ToInt32(a.get_accState(0)); }
  public static string Menu(IntPtr hwnd) {
    var a = Haal(hwnd, 0xFFFFFFFD); if (a == null) return null;
    var sb = new StringBuilder();
    for (int i = 1; i <= a.accChildCount; i++) { sb.Append(a.get_accName(i)); sb.Append(" | "); }
    return a.accChildCount + ": " + sb.ToString();
  }
}
"@
    $hwnd = [IntPtr]$venster.Current.NativeWindowHandle
    $menu = [Msaa]::Menu($hwnd)
    "MSAA menubalk: $menu"
    if (-not $menu -or -not $menu.StartsWith("3:")) { "FOUT: de menubalk heeft geen drie menu's"; $fouten++ }
    $knop = [IntPtr]::Zero
    $knoppen = 0
    while (($knop = [Msaa]::FindWindowEx($hwnd, $knop, "Button", $null)) -ne [IntPtr]::Zero) {
        $rol = [Msaa]::Rol($knop, [uint32]4294967292)
        $naam = [Msaa]::Naam($knop, [uint32]4294967292)
        $status = [Msaa]::Status($knop, [uint32]4294967292)
        $kanfocus = ($status -band 0x100000) -ne 0
        "MSAA knop: rol $rol (43 = drukknop), focus mogelijk: $kanfocus, naam: $naam"
        if ($rol -ne 43) { "FOUT: onderdeel is geen drukknop voor de schermlezer"; $fouten++ }
        if ([string]::IsNullOrWhiteSpace($naam)) { "FOUT: knop zonder naam"; $fouten++ }
        if (-not $kanfocus) { "FOUT: knop kan geen focus krijgen"; $fouten++ }
        $knoppen++
    }
    if ($knoppen -lt 2) { "FOUT: $knoppen knoppen gevonden in plaats van 2"; $fouten++ }

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
