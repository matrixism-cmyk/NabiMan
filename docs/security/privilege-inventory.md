# What the NabiMan API does that needs root

Status: 2026-11-11, branch `phase4-privsep`. Companion to
[`docs/commercialization/privileged-service-control.md`](../commercialization/privileged-service-control.md)
(the helper's design) and `backend/src/run_mode.rs` (the table the server
applies). Keep this file and `run_mode::FEATURES` in step.

Today the API runs as root (`run_mode = root`, the default) and does all of
the below itself. `run_mode = service` runs it as an unprivileged user
(`nabiman`): each item then follows its **target**:

- **(a) helper** — a typed operation of `nabiman-privileged-helper` (root,
  Unix socket, kernel peer uid + pinned API process, allowlisted operation
  with validated arguments, fixed argv, bounded runner, one audit line per
  call). *Done* means service mode uses it now; *target* means it is gated
  until the operation exists.
- **(b) capability / group** — works unprivileged once the service user gets
  a group (or the host a setting); no new code.
- **(c) gated / limited** — service mode answers `503` with the reason
  (`run_mode::gate`), or the feature works with the service user's reach only.

Method: `grep` of every `Command::new`, `/etc` write, `setuid`/`runuser`/`su`,
`SO_PEERCRED`, `/proc` and other users' home reads in `backend/src` (tests
excluded), then each call site read.

## Inventory

| # | Operation | Why root | Code path | Target |
|---|---|---|---|---|
| 1 | Start/stop/restart/enable/disable a unit | systemd's D-Bus policy | `service_control.rs` → `privileged::client` | **(a) done** (`Execute`; refused in root mode since 10-05) |
| 2 | Firewall status (`ufw status`, `firewall-cmd --list-all`, `iptables -L`) | CAP_NET_ADMIN / ufw files root-only | `firewall.rs::get_status` | **(a) done** (`FirewallStatus`) |
| 3 | Add a firewall rule | CAP_NET_ADMIN, `/etc/ufw` | `firewall.rs::add_rule` | **(a) done** (`FirewallAdd`, typed port/range/protocol/verdict) |
| 4 | Delete a firewall rule | as above | `firewall.rs::delete_rule` | **(a) done** (`FirewallDelete`, number 1–10000) |
| 5 | Refresh package lists (`apt update`) | writes `/var/lib/apt` | `updates.rs::check_updates` | **(a) done** (`PackageRefresh`; the listing itself stays unprivileged) |
| 6 | Upgrade every package | dpkg/rpm database, `/usr` | `updates.rs::run_upgrade` | **(a) done** (`PackageUpgrade{None}`, 60 min deadline) |
| 7 | Upgrade one package | as above | `updates.rs::upgrade_package` | **(a) done** (`PackageUpgrade{Some}`; name must start alphanumeric) |
| 8 | Install / remove a package | as above | `packages.rs::install_package`, `remove_package` | (a) target — **gated** |
| 9 | Start a terminal/Claude session as another account (`su --pty -l <user>`) | `su` from root without a password | `terminal/claude_start.rs::command`, `claude_socket.rs` | (a) target (helper spawns a per-account tmux/PTY and passes the fd, SCM_RIGHTS) — **gated** (`startable`); the service user itself still works |
| 10 | Office/Hangul preview converters dropped to `nobody` | `setgroups/setgid/setuid` | `terminal/office_preview.rs` (pre_exec) | (a) target (helper starts the converter as a dedicated `nabiman-preview` user) — **gated**: as the API user the converter could read the data dir |
| 11 | Web server sites: write `/etc/nginx/sites-*`, `/etc/apache2/sites-*`, `a2ensite`, `nginx -t`, reload, certbot | `/etc`, reload | `vhost.rs`, `vhost_config.rs` | (a) target (typed vhost spec) — **gated** (writes) |
| 12 | Service config files: edit, version, roll back, validate, restart | `/etc/*`, `systemctl restart` | `config_manager.rs` | (a) target (allowlisted file + validate + service control) — **gated** (writes) |
| 13 | Issue / renew certificates (certbot) | `/etc/letsencrypt`, reload | `ssl.rs`, `ssl_jobs.rs` | (a) target — **gated** (writes) |
| 14 | OS accounts: `useradd`, `userdel`, `chpasswd` | shadow database | `accounts/operations.rs` | (a) target (uid ≥ 1000 only, never root) — **gated** (writes) |
| 15 | Crontabs of other accounts (`crontab -u`) | spool is root:crontab; `crontab` is setgid (lost under NoNewPrivileges, so even one's own fails) | `cron/operations.rs` | (a) target — **gated** (writes); listing shows errors |
| 16 | Block / unblock addresses (`fail2ban-client`, `iptables`, `nft`) | CAP_NET_ADMIN, fail2ban socket | `ip_block.rs` | (a) target (firewall family) — **gated** (writes) |
| 17 | End login sessions (`fuser /dev/ttyX`, `kill`) | other users' processes | `sessions.rs::kill_session` | (a) target — **gated** |
| 18 | Database status/query/backup/restore (`mysql` via root's socket auth, `sudo -u postgres`) | root's unix-socket DB login; `sudo` (setuid, blocked by NoNewPrivileges) | `database.rs`, `database_pipe.rs`, `dns_db.rs` | (b) better: NabiMan's own DB accounts in the secret store — **gated** (writes); status reads fail soft |
| 19 | Claude voice chat as another account (`runuser -u`) | `runuser` | `terminal/claude_voice/launch.rs` | (c) limited — already refuses with "NabiMan runs as X, so it cannot start Claude as Y" |
| 20 | Local terminal shell is root's | the API is root | `terminal/pty_spawn.rs`, `tmux_operations.rs` | (c) limited — shells run as the service user; `sudo`/`su` blocked by NoNewPrivileges |
| 21 | Signal / inspect any process | other users' processes | `processes.rs`, `processes/signal.rs` | (c) limited — own processes only (CAP_KILL would mean any process, not granted) |
| 22 | File manager over the whole filesystem, `chmod` | DAC | `file_manager.rs` | (c) limited — the service user's reach; `/usr`, `/etc` read-only under ProtectSystem |
| 23 | Claude peer scan: other accounts' `~/.claude/sessions/*.json`, `~/.claude.json`, `/proc/<pid>` | other homes are 0700 | `terminal/claude_peers_records.rs`, `claude_start.rs::places` | (c) limited (a target: helper returns sanitized records) |
| 24 | `journalctl` in the log viewer, mail log | journal read access | `logs.rs`, `mail.rs` | **(b)** `systemd-journal` group (or `adm`) |
| 25 | Mail queue (`mailq` → setgid `postqueue`) | `postdrop` group via setgid | `mail.rs` | (c) limited — setgid does not apply under NoNewPrivileges |
| 26 | Notifications through `sendmail` (setgid `postdrop`) | as above | `notifications.rs` | (c) limited — use SMTP/webhook channels in service mode |
| 27 | SSH attack watch (`/var/log/auth.log`, `journalctl -t sshd`) | `adm` / journal | `attack_watch.rs`, `attack_watch.sh` | **(b)** `adm` + `systemd-journal` groups |
| 28 | Containers (`docker` CLI) | `/var/run/docker.sock` | `containers.rs` | **(b)** `docker` group — root-equivalent; grant knowingly or use rootless Podman |
| 29 | Serial bridge PTY given to `NABIMAN_SERIAL_GROUP` (`socat group-late=`) | chgrp to a group the process is not in | `terminal/serial_bridge.rs` | **(b)** service user in that group (`dialout`, or the box's `nabi`) |
| 30 | ICMP ping | raw socket / file capability (lost under NoNewPrivileges) | `diagnostics.rs::ping` | **(b)** `net.ipv4.ping_group_range` covering the service group (Ubuntu's default does) |
| 31 | Listening port | ports < 1024 need CAP_NET_BIND_SERVICE | `main.rs` (`listen`) | **(b)** not needed: default `127.0.0.1:8080` behind the proxy |
| 32 | `SO_PEERCRED` uid → account on `claude.sock`; `/run/nabiman` | creating `/run/nabiman` | `terminal/claude_socket.rs` | **(b)** no root needed for the credential; the unit's `RuntimeDirectory=nabiman` (0755) makes the directory |
| 33 | tmux server for terminals | none (runs as the API user) | `terminal/tmux_operations.rs` | **(b)** `TMUX_TMPDIR=/run/nabiman` so PrivateTmp does not lose sessions on restart; root's existing sessions do not carry over |
| 34 | guacd shared drive (`<data>/guac-drive`) | none; guacd must read it | `terminal/console_drive.rs` | **(b)** guacd's user in the service group, or guacd as the service user |
| 35 | Data directory `/var/lib/nabiman` 0700 root | ownership | `data_dir/` | **(b)** `nabiman:nabiman` 0700 (`StateDirectory=` in the unit) |
| 36 | Pin the API's PID for the helper (`--pin-api`) | writes the root-only pin | unit `ExecStartPost=+` | root step of the unit, not the API |
| 37 | Box launcher installs `/etc/ssh/ssh_config.d/nabiman-syc.conf` | `/etc` | `bin/start-nabiman.sh` (this box only) | root pre-start step of the supervisor program, not the API |

Read-only collectors that need nothing (`df`, `iostat`, `/proc/*`, `ip`, `w`,
`last`, `ps`, `dig`, `ssh`/`sshpass`/`ssh-keygen`, SFTP/FTP, VNC/RDP relays,
MEC/kube clients, backups of the data dir, recordings) are not listed.

## Counts

- **(a) helper, done:** 7 operations (#1–#7: service control, firewall
  status/add/delete, package refresh/upgrade all/upgrade one).
- **(a) helper, target (gated in service mode now):** 10 (#8–#17).
- **(b) capability / group / unit setting:** 11 (#18 as the alternative, #24, #27–#35).
- **(c) gated or limited:** 8 (#18 gated, #19–#23, #25, #26).
- Root steps outside the API: 2 (#36, #37).

Gated routes in service mode (`run_mode::FEATURES`, 503 with the reason):
package install/remove, `/api/vhost` writes, `/api/config/*` writes,
`/api/ssl/*` writes, `/api/accounts` writes, `/api/cron` writes,
`/api/ipblock/ban|unban`, `/api/sessions/kill`, `/api/database/*` writes,
`/api/preview/office`; plus starting a session as another account.

## The helper as it stands

- Admission: `SO_PEERCRED` uid must be the policy's `api_uid` **and** the
  peer pid must match the root-published pin (start ticks, boot id, uid) —
  terminals of the same uid are refused. Version, canonical UUID request id,
  actor id charset, deadline 1–3 s ahead.
- Service control: units and actions listed in `/etc/nabiman/privileged-services.json`
  (root 0600, root-owned parents), rechecked right before the command; durable
  ledger with idempotency (unchanged).
- System operations: approved per family in `/etc/nabiman/privileged-system.json`
  (`{"api_uid":…, "firewall":true, "package_upgrade":true}`; missing file = none),
  rechecked before each command. Typed arguments only (no strings reach argv
  except a package name that must start with a letter/digit and use
  `[A-Za-z0-9-_.+:]`); programs at fixed root-owned absolute paths, never
  `PATH`; `env_clear` + fixed `PATH`/`LC_ALL`; the bounded runner
  (`bounded_command::capture`: deadline, process group killed, output capped);
  replies carry at most 24 KiB of printable output. One system operation at a
  time on its own thread (a long upgrade does not block service control).
- Audit: `/var/lib/nabiman-helper/audit.jsonl` (root 0600), one line per call —
  peer uid/pid, request id, actor, the typed operation, decision
  (`peer_rejected`, `invalid`, `expired`, `denied`, `dispatched`, `completed`,
  `busy`…), state and exit code. A system operation is not dispatched if its
  audit line cannot be written.
- Not a root shell: there is no operation that takes a program, path, option,
  environment or shell text. Requests are capped at 4096 bytes and
  unknown fields/kinds are rejected by the parser.
- Known limits: a compromised API process can still ask for any approved
  operation and name any actor; package upgrades run maintainer scripts from
  the configured repositories as root (that is what an upgrade is); the
  helper's sandbox must be relaxed for system operations (see
  `packaging/nabiman-privileged-helper.service.d/system-operations.conf`).

## Found on the way

- Fixed: `updates.rs::upgrade_package` and `packages.rs` accepted a package
  name that starts with `-` (e.g. `--allow-unauthenticated`), so an option
  reached `apt`/`dnf` in root mode. Both now use the helper's validator
  (`system_ops::valid_package`) and put `--` before the name (and before a
  search query).
- `firewall.rs::add_rule` with firewalld ignores `deny` and opens the port.
  The helper refuses `deny` on firewalld.
