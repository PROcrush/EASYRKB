//! Standalone restore helpers embedded in every backup archive.
use std::path::Path;

pub const SHELL_NAME: &str = "restore-rekordbox.sh";
pub const POWERSHELL_NAME: &str = "restore-rekordbox.ps1";

fn shell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn powershell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub fn shell(master_db: &Path) -> String {
    let default_db = shell_literal(&master_db.to_string_lossy());
    format!(
        r#"#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${{BASH_SOURCE[0]}}")" && pwd -P)"
DEFAULT_MASTER_DB={default_db}
MASTER_DB="${{1:-$DEFAULT_MASTER_DB}}"
DB_DIR="$(dirname -- "$MASTER_DB")"
SHARE_DIR="$DB_DIR/share"
STAMP="$(date +%Y%m%d-%H%M%S)"
ROLLBACK="$DB_DIR/.rbxport-before-restore-$STAMP-$$"

echo "EASYRKB standalone rekordbox restore"
echo "Backup folder: $SCRIPT_DIR"
echo "Checking the extracted backup contents…"
if [[ ! -f "$SCRIPT_DIR/master.db" || ! -d "$SCRIPT_DIR/analysis" || ! -d "$SCRIPT_DIR/artwork" ]]; then
  echo "This script must be run from an extracted EASYRKB backup." >&2
  exit 1
fi
echo "Backup contents found."
echo "Checking whether rekordbox is running…"
if pgrep -ix rekordbox >/dev/null 2>&1 || pgrep -ix rekordboxAgent >/dev/null 2>&1; then
  echo "Quit rekordbox before restoring this backup." >&2
  exit 1
fi
echo "rekordbox is not running."

echo "This will replace the rekordbox library at:"
echo "  $MASTER_DB"
read -r -p "Continue? [y/N] " answer
case "$answer" in [yY]|[yY][eE][sS]) ;; *) echo "Restore cancelled."; exit 0 ;; esac

echo "Creating rollback folder: $ROLLBACK"
mkdir -p -- "$ROLLBACK/database" "$ROLLBACK/share/PIONEER"
failed() {{
  echo "Restore failed. The previous files are preserved at:" >&2
  echo "  $ROLLBACK" >&2
}}
trap failed ERR

replace_file() {{
  local source="$1" target="$2" saved="$3" temp="${{2}}.rbxport-new-$$"
  echo "Staging file: $source -> $temp"
  mkdir -p -- "$(dirname -- "$target")" "$(dirname -- "$saved")"
  rm -rf -- "$temp"
  cp -p -- "$source" "$temp"
  if [[ -e "$target" ]]; then
    echo "Preserving current file: $target -> $saved"
    mv -- "$target" "$saved"
  else
    echo "No current file to preserve: $target"
  fi
  echo "Installing file: $temp -> $target"
  mv -- "$temp" "$target"
}}
replace_tree() {{
  local source="$1" target="$2" saved="$3" temp="${{2}}.rbxport-new-$$"
  echo "Staging folder: $source -> $temp"
  mkdir -p -- "$(dirname -- "$target")" "$(dirname -- "$saved")"
  rm -rf -- "$temp"
  cp -Rp -- "$source" "$temp"
  if [[ -e "$target" ]]; then
    echo "Preserving current folder: $target -> $saved"
    mv -- "$target" "$saved"
  else
    echo "No current folder to preserve: $target"
  fi
  echo "Installing folder: $temp -> $target"
  mv -- "$temp" "$target"
}}
save_and_remove() {{
  local target="$1" saved="$2"
  if [[ -e "$target" ]]; then
    echo "Preserving stale SQLite sidecar: $target -> $saved"
    mkdir -p -- "$(dirname -- "$saved")"
    mv -- "$target" "$saved"
  else
    echo "No stale SQLite sidecar found: $target"
  fi
}}

echo "Restoring the rekordbox database…"
replace_file "$SCRIPT_DIR/master.db" "$MASTER_DB" "$ROLLBACK/database/master.db"
echo "Removing stale SQLite sidecars…"
save_and_remove "$MASTER_DB-wal" "$ROLLBACK/database/master.db-wal"
save_and_remove "$MASTER_DB-shm" "$ROLLBACK/database/master.db-shm"
echo "Restoring analysis files…"
replace_tree "$SCRIPT_DIR/analysis" "$SHARE_DIR/PIONEER/USBANLZ" "$ROLLBACK/share/PIONEER/USBANLZ"
echo "Restoring artwork…"
replace_tree "$SCRIPT_DIR/artwork" "$SHARE_DIR/PIONEER/Artwork" "$ROLLBACK/share/PIONEER/Artwork"
echo "Restoring library playlist settings…"
for name in masterPlaylists6.xml automixPlaylist6.xml; do
  if [[ -f "$SCRIPT_DIR/$name" ]]; then
    replace_file "$SCRIPT_DIR/$name" "$DB_DIR/$name" "$ROLLBACK/database/$name"
  else
    echo "Optional file is not in this backup; leaving the current file unchanged: $name"
  fi
done

trap - ERR
echo "rekordbox was restored successfully."
echo "The previous files are saved at: $ROLLBACK"
"#
    )
}

pub fn powershell(master_db: &Path) -> String {
    let default_db = powershell_literal(&master_db.to_string_lossy());
    format!(
        r#"param([string]$MasterDb = {default_db})
$ErrorActionPreference = 'Stop'
$SourceDir = $PSScriptRoot
$DbDir = Split-Path -Parent $MasterDb
$ShareDir = Join-Path $DbDir 'share'
$Stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$Rollback = Join-Path $DbDir ".rbxport-before-restore-$Stamp-$PID"

Write-Host 'EASYRKB standalone rekordbox restore'
Write-Host "Backup folder: $SourceDir"
Write-Host 'Checking the extracted backup contents…'
if (!(Test-Path -LiteralPath (Join-Path $SourceDir 'master.db') -PathType Leaf) -or
    !(Test-Path -LiteralPath (Join-Path $SourceDir 'analysis') -PathType Container) -or
    !(Test-Path -LiteralPath (Join-Path $SourceDir 'artwork') -PathType Container)) {{
  throw 'This script must be run from an extracted EASYRKB backup.'
}}
Write-Host 'Backup contents found.'
Write-Host 'Checking whether rekordbox is running…'
if (Get-Process -Name rekordbox, rekordboxAgent -ErrorAction SilentlyContinue) {{
  throw 'Quit rekordbox before restoring this backup.'
}}
Write-Host 'rekordbox is not running.'

Write-Host 'This will replace the rekordbox library at:'
Write-Host "  $MasterDb"
$Answer = Read-Host 'Continue? [y/N]'
if ($Answer -notmatch '^(?i:y|yes)$') {{ Write-Host 'Restore cancelled.'; exit 0 }}

Write-Host "Creating rollback folder: $Rollback"
New-Item -ItemType Directory -Force -Path (Join-Path $Rollback 'database'), (Join-Path $Rollback 'share/PIONEER') | Out-Null

function Replace-File([string]$Source, [string]$Target, [string]$Saved) {{
  $Temp = "$Target.rbxport-new-$PID"
  Write-Host "Staging file: $Source -> $Temp"
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Target), (Split-Path -Parent $Saved) | Out-Null
  Remove-Item -LiteralPath $Temp -Recurse -Force -ErrorAction SilentlyContinue
  Copy-Item -LiteralPath $Source -Destination $Temp -Force
  if (Test-Path -LiteralPath $Target) {{
    Write-Host "Preserving current file: $Target -> $Saved"
    Move-Item -LiteralPath $Target -Destination $Saved
  }} else {{
    Write-Host "No current file to preserve: $Target"
  }}
  Write-Host "Installing file: $Temp -> $Target"
  Move-Item -LiteralPath $Temp -Destination $Target
}}
function Replace-Tree([string]$Source, [string]$Target, [string]$Saved) {{
  $Temp = "$Target.rbxport-new-$PID"
  Write-Host "Staging folder: $Source -> $Temp"
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Target), (Split-Path -Parent $Saved) | Out-Null
  Remove-Item -LiteralPath $Temp -Recurse -Force -ErrorAction SilentlyContinue
  Copy-Item -LiteralPath $Source -Destination $Temp -Recurse -Force
  if (Test-Path -LiteralPath $Target) {{
    Write-Host "Preserving current folder: $Target -> $Saved"
    Move-Item -LiteralPath $Target -Destination $Saved
  }} else {{
    Write-Host "No current folder to preserve: $Target"
  }}
  Write-Host "Installing folder: $Temp -> $Target"
  Move-Item -LiteralPath $Temp -Destination $Target
}}
function Save-AndRemove([string]$Target, [string]$Saved) {{
  if (Test-Path -LiteralPath $Target) {{
    Write-Host "Preserving stale SQLite sidecar: $Target -> $Saved"
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Saved) | Out-Null
    Move-Item -LiteralPath $Target -Destination $Saved
  }} else {{
    Write-Host "No stale SQLite sidecar found: $Target"
  }}
}}

try {{
  Write-Host 'Restoring the rekordbox database…'
  Replace-File (Join-Path $SourceDir 'master.db') $MasterDb (Join-Path $Rollback 'database/master.db')
  Write-Host 'Removing stale SQLite sidecars…'
  Save-AndRemove "$MasterDb-wal" (Join-Path $Rollback 'database/master.db-wal')
  Save-AndRemove "$MasterDb-shm" (Join-Path $Rollback 'database/master.db-shm')
  Write-Host 'Restoring analysis files…'
  Replace-Tree (Join-Path $SourceDir 'analysis') (Join-Path $ShareDir 'PIONEER/USBANLZ') (Join-Path $Rollback 'share/PIONEER/USBANLZ')
  Write-Host 'Restoring artwork…'
  Replace-Tree (Join-Path $SourceDir 'artwork') (Join-Path $ShareDir 'PIONEER/Artwork') (Join-Path $Rollback 'share/PIONEER/Artwork')
  Write-Host 'Restoring library playlist settings…'
  foreach ($Name in 'masterPlaylists6.xml', 'automixPlaylist6.xml') {{
    $Source = Join-Path $SourceDir $Name
    if (Test-Path -LiteralPath $Source -PathType Leaf) {{
      Replace-File $Source (Join-Path $DbDir $Name) (Join-Path $Rollback "database/$Name")
    }} else {{
      Write-Host "Optional file is not in this backup; leaving the current file unchanged: $Name"
    }}
  }}
}} catch {{
  Write-Error "Restore failed. The previous files are preserved at: $Rollback`n$_"
  exit 1
}}

Write-Host 'rekordbox was restored successfully.'
Write-Host "The previous files are saved at: $Rollback"
"#
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_quoted_for_each_shell() {
        let path = Path::new("/tmp/Chris's library/master.db");
        assert!(shell(path).contains("'/tmp/Chris'\\''s library/master.db'"));
        assert!(powershell(path).contains("'/tmp/Chris''s library/master.db'"));
    }

    #[cfg(unix)]
    #[test]
    fn shell_script_restores_every_payload_and_preserves_previous_files() {
        use std::{
            fs,
            io::Write,
            process::{Command, Stdio},
        };

        let dir = tempfile::tempdir().unwrap();
        let extracted = dir.path().join("extracted");
        let live = dir.path().join("live");
        let database = live.join("master.db");
        fs::create_dir_all(extracted.join("analysis")).unwrap();
        fs::create_dir_all(extracted.join("artwork")).unwrap();
        fs::create_dir_all(live.join("share/PIONEER/USBANLZ")).unwrap();
        fs::create_dir_all(live.join("share/PIONEER/Artwork")).unwrap();
        fs::write(extracted.join("master.db"), b"saved database").unwrap();
        fs::write(extracted.join("analysis/saved.DAT"), b"saved analysis").unwrap();
        fs::write(extracted.join("artwork/saved.jpg"), b"saved artwork").unwrap();
        fs::write(extracted.join("masterPlaylists6.xml"), b"saved playlists").unwrap();
        fs::write(&database, b"current database").unwrap();
        fs::write(live.join("master.db-wal"), b"current wal").unwrap();
        fs::write(
            live.join("share/PIONEER/USBANLZ/current.DAT"),
            b"current analysis",
        )
        .unwrap();
        fs::write(
            live.join("share/PIONEER/Artwork/current.jpg"),
            b"current artwork",
        )
        .unwrap();
        fs::write(live.join("masterPlaylists6.xml"), b"current playlists").unwrap();
        let script = extracted.join(SHELL_NAME);
        fs::write(&script, shell(&database)).unwrap();

        let mut child = Command::new("bash")
            .arg(&script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"yes\n").unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        for message in [
            "Checking the extracted backup contents",
            "Checking whether rekordbox is running",
            "Creating rollback folder",
            "Restoring the rekordbox database",
            "Preserving current file",
            "Removing stale SQLite sidecars",
            "Restoring analysis files",
            "Restoring artwork",
            "Restoring library playlist settings",
            "Optional file is not in this backup",
            "rekordbox was restored successfully",
        ] {
            assert!(
                stdout.contains(message),
                "missing {message:?} in:\n{stdout}"
            );
        }
        assert_eq!(fs::read(&database).unwrap(), b"saved database");
        assert!(!live.join("master.db-wal").exists());
        assert_eq!(
            fs::read(live.join("share/PIONEER/USBANLZ/saved.DAT")).unwrap(),
            b"saved analysis"
        );
        assert!(!live.join("share/PIONEER/USBANLZ/current.DAT").exists());
        assert_eq!(
            fs::read(live.join("share/PIONEER/Artwork/saved.jpg")).unwrap(),
            b"saved artwork"
        );
        assert_eq!(
            fs::read(live.join("masterPlaylists6.xml")).unwrap(),
            b"saved playlists"
        );
        let rollback = fs::read_dir(&live)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".rbxport-before-restore-")
            })
            .unwrap();
        assert_eq!(
            fs::read(rollback.join("database/master.db")).unwrap(),
            b"current database"
        );
        assert_eq!(
            fs::read(rollback.join("database/master.db-wal")).unwrap(),
            b"current wal"
        );
    }
}
