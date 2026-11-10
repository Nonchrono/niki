# Loaded by the installer and by the isolated shortcut tests.
function Update-NikiShortcutIcons {
    param(
        [Parameter(Mandatory=$true)]$Shell,
        [Parameter(Mandatory=$true)][string]$Executable,
        [Parameter(Mandatory=$true)][string]$Icon,
        [string[]]$Folders
    )
    foreach ($folder in $Folders) {
        if (-not $folder -or -not (Test-Path -LiteralPath $folder)) { continue }
        foreach ($file in Get-ChildItem -LiteralPath $folder -Filter '*.lnk' -File -ErrorAction SilentlyContinue) {
            try {
                $link = $Shell.CreateShortcut($file.FullName)
                # Never change another application's pins or create new pins.
                if (-not [string]::Equals($link.TargetPath, $Executable, [StringComparison]::OrdinalIgnoreCase)) { continue }
                $link.IconLocation = "$Icon,0"
                $link.Save()
                $file.FullName
            } catch {
                # A stale or protected pin must not prevent the app update.
                Write-Warning "Could not refresh a Niki shortcut: $($_.Exception.Message)"
            }
        }
    }
}

function Send-NikiIconChange {
    param([string[]]$Paths)
    if (-not ('Niki.ShellIconNotifications' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace Niki {
    public static class ShellIconNotifications {
        [DllImport("shell32.dll", EntryPoint="SHChangeNotify", CharSet=CharSet.Unicode)]
        public static extern void UpdateItem(int eventId, uint flags, string path, IntPtr unused);
        [DllImport("shell32.dll", EntryPoint="SHChangeNotify")]
        public static extern void Invalidate(int eventId, uint flags, IntPtr first, IntPtr second);
    }
}
'@
    }
    # SHCNE_ASSOCCHANGED + SHCNF_IDLIST | SHCNF_FLUSHNOWAIT invalidates cached icons.
    [Niki.ShellIconNotifications]::Invalidate(0x08000000, 0x2000, [IntPtr]::Zero, [IntPtr]::Zero)
    foreach ($path in $Paths | Select-Object -Unique) {
        if ($path -and (Test-Path -LiteralPath $path)) {
            # Windows 11 also needs item notifications for existing taskbar pins.
            # SHCNE_UPDATEITEM + SHCNF_PATHW | SHCNF_FLUSHNOWAIT.
            [Niki.ShellIconNotifications]::UpdateItem(0x2000, 0x2005, $path, [IntPtr]::Zero)
        }
    }
}
