# NabiMan: what Claude Code is running here, for the session tiles (Windows).
# Same output as claude_remote_probe.sh; offsets come in as $offsets (sessionId -> offset).
# Only addresses, times, ids and each message's first line leave the machine.
$ErrorActionPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$procs = @{}
Get-CimInstance Win32_Process | ForEach-Object { $procs[[int]$_.ProcessId] = $_ }
function Parent([int]$p) { if ($procs.ContainsKey($p)) { [int]$procs[$p].ParentProcessId } else { 0 } }
# The sshd for this connection: the one just below the topmost sshd (the listener).
$conn = 0; $below = 0; $p = $PID; $n = 0
while ($p -gt 0 -and $procs.ContainsKey($p) -and $n -lt 64) {
  if ($procs[$p].Name -like 'sshd*') { $conn = $below; $below = $p }
  $p = Parent $p; $n++
}
if ($conn -eq 0) { $conn = $below }
"C`t$conn"
function Under([int]$q) {
  $n = 0
  while ($q -gt 0 -and $n -lt 64) { if ($q -eq $conn) { return $true }; if (-not $procs.ContainsKey($q)) { return $false }; $q = Parent $q; $n++ }
  return $false
}
$home_ = $env:USERPROFILE
Get-ChildItem (Join-Path $home_ '.claude\sessions') -Filter *.json | ForEach-Object {
  $raw = [IO.File]::ReadAllText($_.FullName)
  try { $rec = $raw | ConvertFrom-Json } catch { return }
  $id = [int]$rec.pid
  if (-not $procs.ContainsKey($id) -or $procs[$id].Name -notlike 'claude*') { return }
  $in = if (Under $id) { 1 } else { 0 }
  "R`t$in`t" + [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($raw -replace "`r?`n", '')))
  $sid = [string]$rec.sessionId
  if ($sid -notmatch '^[0-9a-f-]+$') { return }
  $t = Get-ChildItem (Join-Path $home_ '.claude\projects') -Directory | ForEach-Object { Join-Path $_.FullName "$sid.jsonl" } | Where-Object { Test-Path $_ } | Select-Object -First 1
  if (-not $t) { return }
  $size = (Get-Item $t).Length
  $off = 0; if ($offsets.ContainsKey($sid)) { $off = [int64]$offsets[$sid] }
  if ($off -gt $size) { $off = 0 }
  if ($size -gt $off) {
    $fs = [IO.File]::Open($t, 'Open', 'Read', 'ReadWrite')
    try {
      [void]$fs.Seek($off, 'Begin')
      $sr = New-Object IO.StreamReader($fs, [Text.Encoding]::UTF8)
      $left = $size - $off
      while (-not $sr.EndOfStream) {
        $line = $sr.ReadLine()
        if ($line.IndexOf('"kind":"peer"') -lt 0) { continue }
        $from = [regex]::Match($line, '"from":"((?:[^"\\]|\\.)*)"').Groups[1].Value -replace '\\\\', '\'
        $at = [regex]::Match($line, '"timestamp":"([^"]*)"').Groups[1].Value
        $mid = [regex]::Match($line, '"msg_id":"([^"]*)"').Groups[1].Value
        $said = [regex]::Match($line, '<cross-session-message[^>]*>\\n((?:[^\\]|\\")*)').Groups[1].Value
        if ($said.Length -gt 400) { $said = $said.Substring(0, 400) }
        $said = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($said))
        if ($from -and $at) { "M`t$sid`t$from`t$at`t$mid`t$said" }
      }
    } finally { $fs.Close() }
  }
  "O`t$sid`t$size"
}
