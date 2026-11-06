# NabiMan: what Claude Code is running here, for the session tiles.
# Runs over the tile's own SSH connection (ControlMaster), as the login user.
# Prints, one per line (tab-separated):
#   C <pid of this SSH connection's sshd>
#   R <1 if under that connection else 0> <base64 of the session record>
#   O <session id> <transcript size>
#   M <session id> <from> <timestamp> <msg_id> <base64 first line>   (peer messages after the given offset)
# Only addresses, times, ids and each message's first line leave the machine.
# Offsets come in as lines "<session id> <offset>" in NABIMAN_OFFSETS.
export LC_ALL=C
parent() { awk '{ sub(/^.*\) /, ""); print $2 }' "/proc/$1/stat" 2>/dev/null; }
starttime() { awk '{ sub(/^.*\) /, ""); print $20 }' "/proc/$1/stat" 2>/dev/null; }
is_sshd() { case "$(cat "/proc/$1/comm" 2>/dev/null)" in sshd*) return 0;; esac; return 1; }
# The sshd for this connection: the one just below the topmost sshd (the listener).
conn=""; below=""; p=$$
while [ -n "$p" ] && [ "$p" -gt 1 ]; do
  if is_sshd "$p"; then conn=$below; below=$p; fi
  p=$(parent "$p")
done
[ -n "$conn" ] || conn=$below
printf 'C\t%s\n' "$conn"
under() { q=$1; n=0; while [ -n "$q" ] && [ "$q" -gt 1 ] && [ $n -lt 64 ]; do [ "$q" = "$conn" ] && return 0; q=$(parent "$q"); n=$((n+1)); done; return 1; }
field() { printf '%s' "$1" | grep -o "\"$2\":\"[^\"]*\"" | head -n 1 | cut -d'"' -f4; }
num() { printf '%s' "$1" | grep -o "\"$2\":[0-9]*" | head -n 1 | cut -d: -f2; }
for f in "$HOME"/.claude/sessions/*.json; do
  [ -f "$f" ] || continue
  rec=$(head -c 65536 "$f" | tr -d '\n')
  pid=$(num "$rec" pid)
  [ -n "$pid" ] && [ -d "/proc/$pid" ] || continue
  [ "$(starttime "$pid")" = "$(field "$rec" procStart)" ] || continue
  in=0; under "$pid" && in=1
  printf 'R\t%s\t%s\n' "$in" "$(printf '%s' "$rec" | base64 | tr -d '\n')"
  sid=$(field "$rec" sessionId)
  case "$sid" in *[!0-9a-f-]*|"") continue;; esac
  t=$(ls "$HOME"/.claude/projects/*/"$sid".jsonl 2>/dev/null | head -n 1)
  [ -n "$t" ] || continue
  size=$(wc -c < "$t" | tr -d ' ')
  off=$(printf '%s\n' "$NABIMAN_OFFSETS" | awk -v s="$sid" '$1 == s { print $2 }' | head -n 1)
  case "$off" in ''|*[!0-9]*) off=0;; esac
  [ "$off" -gt "$size" ] && off=0
  if [ "$size" -gt "$off" ]; then
    tail -c +"$((off + 1))" "$t" | head -c "$((size - off))" | grep -a '"kind":"peer"' | while IFS= read -r line; do
      from=$(field "$line" from); at=$(field "$line" timestamp); id=$(field "$line" msg_id)
      # The first line it said (up to the first escape), for the arrow's tooltip.
      said=$(printf '%s' "$line" | sed -n 's/.*<cross-session-message[^>]*>\\n\([^\\]*\).*/\1/p' | head -c 400 | base64 | tr -d '\n')
      [ -n "$from" ] && [ -n "$at" ] && printf 'M\t%s\t%s\t%s\t%s\t%s\n' "$sid" "$from" "$at" "$id" "$said"
    done
  fi
  printf 'O\t%s\t%s\n' "$sid" "$size"
done
