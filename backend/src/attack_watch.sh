# Prints this machine's SSH login attempts of the last $1 minutes, for
# attack_watch.rs. Read-only. Lines:
#   SRC <journal | journal-sudo | /var/log/… | none>   where the log was read
#   CAN <yes|no>                                        whether it could be read
#   NOW <seconds since 1970>
#   <log lines>     the journal's start with seconds since 1970; a file's as written
#   BAN <ip> <ip>…  whom fail2ban holds off (empty when it does not run or may not be asked)
W=${1:-60}
case "$W" in '' | *[!0-9]*) W=60 ;; esac
NOW=$(date +%s)
src=none
can=no
out=""
pick() { grep -a 'sshd' | grep -aE 'Failed password|Invalid user|Accepted |authenticating user' | tail -n 20000; }
may_sudo() { command -v sudo >/dev/null 2>&1 && sudo -n true 2>/dev/null; }

if command -v journalctl >/dev/null 2>&1; then
  J="journalctl -q --no-pager -o short-unix --since=-${W}min -t sshd -t sshd-session"
  member=no
  case " $(id -Gn 2>/dev/null) " in *" adm "* | *" systemd-journal "* | *" wheel "*) member=yes ;; esac
  if [ "$(id -u)" = 0 ] || [ "$member" = yes ]; then
    out=$($J 2>/dev/null | pick)
    src=journal
    can=yes
  elif may_sudo; then
    out=$(sudo -n $J 2>/dev/null | pick)
    src=journal-sudo
    can=yes
  fi
fi

if [ -z "$out" ]; then
  since=$(date -d "-$W min" +%Y-%m-%dT%H:%M:%S 2>/dev/null)
  for f in /var/log/auth.log /var/log/secure; do
    [ -f "$f" ] || continue
    if [ -r "$f" ]; then
      raw=$(tail -n 50000 "$f" 2>/dev/null)
    elif may_sudo; then
      raw=$(sudo -n tail -n 50000 "$f" 2>/dev/null)
    else
      [ "$can" = yes ] || src=$f
      continue
    fi
    # Lines stamped 2026-11-10T08:00:00… are kept from the window on; older styles of stamp are all kept.
    out=$(printf '%s\n' "$raw" | awk -v s="$since" '{ if (s != "" && $1 ~ /^[0-9][0-9][0-9][0-9]-/) { if (substr($1, 1, 19) >= s) print } else print }' | pick)
    src=$f
    can=yes
    break
  done
fi

printf 'SRC %s\nCAN %s\nNOW %s\n' "$src" "$can" "$NOW"
[ -n "$out" ] && printf '%s\n' "$out"
ban=""
if command -v fail2ban-client >/dev/null 2>&1; then
  ban=$( (fail2ban-client status sshd 2>/dev/null || { may_sudo && sudo -n fail2ban-client status sshd 2>/dev/null; }) | sed -n 's/.*Banned IP list:[[:space:]]*//p')
fi
printf 'BAN %s\n' "$ban"
