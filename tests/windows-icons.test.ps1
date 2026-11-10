$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\src\installer\windows_icons.ps1')

# Real .lnk files in an isolated directory; never edit the user's pins or registry.
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('niki-icons-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot | Out-Null
try {
    $shell = New-Object -ComObject WScript.Shell
    $executable = Join-Path $testRoot 'Niki.exe'
    $oldIcon = Join-Path $testRoot 'icon.ico'
    $newIcon = Join-Path $testRoot 'icon-new.ico'
    [IO.File]::WriteAllBytes($newIcon, [byte[]]@(0))
    $nikiLink = Join-Path $testRoot 'Renamed Niki.lnk'
    $otherLink = Join-Path $testRoot 'Other app.lnk'
    foreach ($linkPath in @($nikiLink, $otherLink)) {
        $link = $shell.CreateShortcut($linkPath)
        $link.TargetPath = if ($linkPath -eq $nikiLink) { $executable.ToUpperInvariant() } else { Join-Path $testRoot 'Other.exe' }
        $link.Arguments = '--example'
        $link.IconLocation = "$oldIcon,0"
        $link.Save()
    }
    $otherHash = (Get-FileHash -LiteralPath $otherLink).Hash
    $changed = @(Update-NikiShortcutIcons -Shell $shell -Executable $executable -Icon $newIcon -Folders @($testRoot, (Join-Path $testRoot 'Missing')))
    if ($changed.Count -ne 1 -or $changed[0] -ne $nikiLink) { throw 'Only the matching Niki pin should be refreshed.' }
    $updated = $shell.CreateShortcut($nikiLink)
    if ($updated.IconLocation -ne "$newIcon,0") { throw 'Pin still uses the old icon.' }
    if ($updated.Arguments -ne '--example' -or $updated.TargetPath -ine $executable) { throw 'Shortcut target or arguments changed.' }
    if ((Get-FileHash -LiteralPath $otherLink).Hash -ne $otherHash) { throw 'Another application shortcut was modified.' }
    if (@(Get-ChildItem -LiteralPath $testRoot -Filter '*.lnk').Count -ne 2) { throw 'The refresh created a new pin.' }

    # Capture notification calls without invalidating this machine's actual cache.
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
namespace Niki {
    public static class ShellIconNotifications {
        public static List<string> Paths = new List<string>();
        public static int Invalidations;
        public static void UpdateItem(int eventId, uint flags, string path, IntPtr unused) {
            if (eventId != 0x2000 || flags != 0x2005 || unused != IntPtr.Zero) throw new Exception("Wrong item notification");
            Paths.Add(path);
        }
        public static void Invalidate(int eventId, uint flags, IntPtr first, IntPtr second) {
            if (eventId != 0x08000000 || flags != 0x2000 || first != IntPtr.Zero || second != IntPtr.Zero) throw new Exception("Wrong cache notification");
            Invalidations++;
        }
    }
}
'@
    Send-NikiIconChange -Paths @($nikiLink, $nikiLink, $newIcon, (Join-Path $testRoot 'missing.ico'))
    if ([Niki.ShellIconNotifications]::Invalidations -ne 1 -or [Niki.ShellIconNotifications]::Paths.Count -ne 2) { throw 'Missing or duplicate cache notifications.' }
    Write-Host 'Windows icon refresh: matching pins, preserved targets, unrelated shortcuts and cache notifications passed.'
} finally {
    $resolvedTestRoot = [IO.Path]::GetFullPath($testRoot)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (-not $resolvedTestRoot.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($resolvedTestRoot) -notlike 'niki-icons-test-*') { throw 'Unexpected test cleanup path.' }
    Remove-Item -LiteralPath $resolvedTestRoot -Recurse -Force
}
