# NabiMan Admin Guide

## Multi-User Management

### Creating Users
1. Log in as Admin
2. Go to Security > User Management
3. Click "Add User"
4. Set username, password (min 8 chars), and role

### Role-Based Access Control (RBAC)
- **Admin**: Full access. Can manage users, accounts, firewall rules.
- **Operator**: Can restart services, manage containers, packages. Cannot manage users or host accounts.
- **Viewer**: Read-only. Can view all monitoring data but cannot make changes.

## Two-Factor Authentication

### Enabling 2FA
- Call `POST /api/auth/2fa/setup` to generate a TOTP secret
- Scan the QR code URI with an authenticator app
- Verify with `POST /api/auth/2fa/verify-setup` using a valid code
- 2FA is now active for that user

### Login with 2FA
1. Submit username + password
2. Server responds with `requires_2fa: true`
3. Submit TOTP code to `/api/auth/2fa/verify`
4. Receive JWT tokens

### Disabling 2FA
- Call `POST /api/auth/2fa/disable` with current password

## Session Management
- Access tokens expire in 15 minutes
- Refresh tokens expire in 7 days
- View active sessions: `GET /api/auth/sessions`
- Revoke a session: `POST /api/auth/sessions/revoke`
- Revoke all sessions: `POST /api/auth/sessions/revoke-all`

## Alert Rules
1. Go to Management > Alert Rules
2. Create rules for CPU, Memory, or Disk thresholds
3. Set up notification channels (Webhook, Slack, Email)
4. Alerts trigger automatically with configurable cooldown

## Audit Log
- All API calls are automatically logged (except high-frequency polling)
- View at Security > Audit Log
- Filter by user, method, or path
- Log rotation at 10MB (keeps 5 rotated files)

## Backup
- Backup config files at Management > Backup
- Backups stored in `$NABIMAN_DATA_DIR/backups/`

## Data Directory Structure
```
/var/lib/nabiman/
├── users.json          # User accounts (bcrypt hashed passwords)
├── jwt_secret          # JWT signing key
├── password            # Legacy password file (bcrypt)
├── remote_servers.json # Remote server inventory
├── notifications.json  # Notification channels
├── alert_rules.json    # Alert rule definitions
├── audit.jsonl         # Audit log
├── audit.jsonl.1-5     # Rotated audit logs
└── backups/            # Config file backups
```
