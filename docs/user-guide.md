# NabiMan User Guide

## Installation

### Quick Install (Binary)
```bash
curl -sSL https://raw.githubusercontent.com/matrixism-cmyk/NabiMan/main/install.sh | sudo bash
```

### Build from Source
```bash
git clone https://github.com/matrixism-cmyk/NabiMan.git
cd NabiMan
make build
sudo make install
```

### Docker
```bash
docker-compose up -d
```

## First Login
- Access: `http://your-server:8080`
- Default username: `admin`
- Default password: `nabiman`
- **Change the password immediately after first login**

## User Roles
| Role | Permissions |
|------|------------|
| Admin | Full access including user management |
| Operator | Read + operational tasks (restart services, manage containers) |
| Viewer | Read-only access to all monitoring data |

## Two-Factor Authentication (2FA)
1. Go to Security > User Management
2. Set up 2FA using Google Authenticator or similar TOTP app
3. On next login, enter your 6-digit TOTP code after password

## Configuration
Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `NABIMAN_PORT` | `8080` | Server port |
| `NABIMAN_PASSWORD` | `nabiman` | Initial admin password |
| `NABIMAN_STATIC` | `./static` | Frontend static files path |
| `NABIMAN_DATA_DIR` | `/var/lib/nabiman` | Data directory |
| `NABIMAN_CORS_ORIGINS` | (any) | Comma-separated allowed origins |
| `NABIMAN_HSTS` | `false` | Enable HSTS header |

## HTTPS Setup
Use a reverse proxy (Nginx, Apache, Caddy) with Let's Encrypt:
```
# Apache example
ProxyPass / http://127.0.0.1:8080/
ProxyPassReverse / http://127.0.0.1:8080/
```

## Remote Servers
Remote → Remote Servers (`#remote/remote`) keeps the list of machines you manage.

### Saved passwords
- Pick **Auth: Password** when adding a server to store its SSH password.
- The password is encrypted with AES-256-GCM before it is written to
  `$NABIMAN_DATA_DIR/remote_servers.json`; the key lives in
  `$NABIMAN_DATA_DIR/secret.key` (mode 0600) and never leaves the server.
- The API only ever reports `has_password` — the password itself is never sent
  back to a browser.
- Stored passwords are used automatically for **Check**, **Exec** and terminal
  sessions (via `sshpass -f`, so the password never appears in `ps` or in the
  environment). Edit a server to replace it, or **Clear password** to remove it.
- SSH keys remain the recommended method; **Deploy Key** can install the
  NabiMan key using the stored password.

### Editing a server
**Edit** exposes every field the add form has — name, host, port, user, auth
method, password, tags and memo — and validates host/user/port before saving.
Leaving the password box empty keeps the stored password; **Clear password**
removes it. **Deploy Key** also works with an empty password box when a password
is already saved.

### How a server is reached
A server has a **terminal** protocol (SSH, Telnet or none) and a **files**
protocol (SFTP, FTPS, FTP or none), chosen separately, since SSH-only, SFTP-only
and FTP-only machines are all common. Records saved before this are SSH + SFTP.
- The list shows a badge per protocol; Telnet and FTP, which send passwords
  unencrypted, are marked in orange. Once the list holds more than one kind,
  a protocol filter appears beside the search box.
- A server offers only the views it supports: no terminal means no sessions or
  terminal tab, and its files open as a full-page **파일** view; commands
  (**실행**) and CPU/memory figures need an SSH shell.
- **접속 방식 확인** in the server settings tries the SSH shell and SFTP one by
  one (and, for plain FTP, whether TLS works) and sets the protocols to what
  works. It runs by itself once for a new server, and for an SSH server again
  when its host key is approved. Correct the result in the form if needed.
- **Check** tests every protocol the server is set up for and lists each result;
  a server where some work and some do not is shown as partly reachable.
- **Telnet** sessions run NabiMan's own client (the server binary started as
  `nabiman-server telnet-client`, so no telnet package is needed). It reports
  the window size and terminal type, types the saved user at the login prompt
  and the stored password at the password prompt, once each.
- **FTPS** is explicit TLS (`AUTH TLS`, usually port 21) or implicit TLS from the
  first byte (usually port 990), chosen in the form. A certificate must either be valid for
  the host name or match the SHA-256 fingerprint saved on the server (paste it
  as `openssl x509 -fingerprint -sha256` prints it); an untrusted certificate is
  refused with its fingerprint in the message, to verify out of band. Data
  connections resume the control connection's TLS session, as vsftpd requires.
- FTP data connections always go to the address the control connection
  reached, never to an address a PASV reply names. FTP users may contain `@`
  (`web@example.com`); SSH users may not.

### File permissions and owners
Right-click an entry in the file panel — or several, selected with Ctrl/Shift —
and pick **권한·소유자 변경…**. Tick the read/write/execute boxes or type the
octal mode (`755`), and give an owner and/or group by name or numeric id; empty
fields are left as they are, and with several entries whose modes differ the
mode is only changed when **권한 바꾸기** is ticked. For folders, **폴더 안의 모든
항목에도 적용** walks everything inside (folders also get `x` wherever they get
`r`, as `chmod -R a+X` does); links are never changed. Over FTP only the mode can
be changed (`SITE CHMOD`); owner names show where the server can tell them (this
machine, SSH servers with a shell, FTP listings), otherwise numeric ids.

### Previewing files
Double-click a file, select it and press Space, or pick **미리보기** from its
right-click menu to see it in place; ← and → (Home/End) step through the
folder's other previewable files.
- **Images** (png, jpg, gif, webp, avif, bmp, ico, svg; up to 50 MB): fitted to
  the window or at actual size (F, or click the picture). An SVG is shown as an
  image, so any script in it never runs.
- **Text and code** (up to 5 MB): line numbers, colouring for common languages
  and config formats (also `Dockerfile`, `Makefile`, `.bashrc`, `nginx.conf`, …),
  wrap long lines with W (remembered), **전체 복사**. The encoding is found
  automatically — UTF-8, or EUC-KR for older Korean files — and can be chosen.
  Files over 300 KB are shown plain, as one block; a binary file asks before
  being shown as text.

### Sharing a server's files
A server reached only for its files has **🔗 파일 공유** on its file view: a link
like a terminal share link (expiry, optional password, revocable in **공유 링크**)
that opens only the file panel, for browsing and downloading or, if chosen, for
uploading and changing too. The link is held to how the server is reached:
changing its address, user, protocol, password, approved host key or
certificate ends the link.

### Terminal windows
- **⧉ Terminal window** on a server card opens a floating terminal for it. Open
  as many as you like: each window is independently draggable and resizable,
  can be minimised, maximised or tiled, and the taskbar at the bottom of the
  screen switches between them (**⊞ 정렬** tiles, **⧉ 계단식** cascades).
- Window positions are remembered across page reloads and each window
  re-attaches to its own session.
- Closing a window with **✕** leaves the session running on the server; **⏻**
  ends the session and the remote shell with it.

### When a connection fails or ends
- If SSH cannot connect (host down, wrong password, host key problem), the pane
  keeps the ssh error on screen with a red `[SSH 연결 종료 · exit N]` line instead
  of vanishing. Press Enter (or use **⏻**) to close it; an unattended failed pane
  closes itself after 15 minutes.
- When a shell exits normally, the server tells the browser the session is over,
  so it does **not** silently reconnect into a new shell. The pane shows
  "세션 종료됨 · 클릭하면 새 세션" — click it to start a fresh one.
- Only a genuine network drop triggers automatic reconnection. A socket that has
  never connected gives up after 8 attempts and offers a manual retry, so a
  broken target can never produce an endless "reconnecting" loop.
- Sessions are created at the browser's current terminal size, so the first lines
  of output are never reflowed off the screen when the pane attaches.

## Terminal Settings
Open with **⚙** in the terminal panel or from the terminal taskbar.

| Setting | Default | Meaning |
|---------|---------|---------|
| Scrollback lines | `5000` | History kept per session — both the browser buffer and tmux's `history-limit`. Range 100–200000. |
| Keep alive | on | The session is never auto-closed while detached, SSH keep-alive packets are sent, and a dropped browser reconnects automatically. |
| Keep-alive interval | `30s` | SSH `ServerAliveInterval` for remote sessions. |
| Idle session lifetime | permanent | Only when keep-alive is off: how long a detached session survives. |
| Restore previous output | on | Re-opening a window replays the history tmux still holds. |
| Font size | `14` | Also adjustable with Ctrl + mouse wheel. |

Settings are stored per user in `$NABIMAN_DATA_DIR/terminal_settings.json`, so
they follow you across browsers.

Sessions themselves run inside tmux and are recorded in
`$NABIMAN_DATA_DIR/terminal_sessions.json`, so they survive a NabiMan restart:
after an upgrade, open windows re-attach to the shells that kept running.
