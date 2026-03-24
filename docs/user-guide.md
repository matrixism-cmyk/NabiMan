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
