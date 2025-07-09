# Installation Guide

This guide covers various methods to install UniFi Protect Backup (Rust Edition).

## Prerequisites

### System Requirements

- **Operating System**: Linux, macOS, or Windows
- **Memory**: Minimum 512MB RAM (2GB+ recommended for high throughput)
- **Storage**: Sufficient space for backup storage plus temporary processing
- **Network**: Access to UniFi Protect controller and backup destinations

### Dependencies

- **Borg Backup** (for archive functionality): `sudo apt install borgbackup` or equivalent
- **Rclone** (for cloud backups): See [rclone.org](https://rclone.org/install/)
- **SSH Client** (for remote Borg repositories)

## Installation Methods

### Option 1: Pre-built Binaries (Recommended)

Download the latest release from GitHub:

```bash
# Linux x86_64
curl -LO https://github.com/stphnsmpsn/unifi-protect-backup-rs/releases/latest/download/unifi-protect-backup-rs-linux-x86_64.tar.gz
tar -xzf unifi-protect-backup-rs-linux-x86_64.tar.gz
sudo mv unifi-protect-backup-rs /usr/local/bin/

# Make executable
sudo chmod +x /usr/local/bin/unifi-protect-backup-rs
```

### Option 2: Cargo Install

If you have Rust installed:

```bash
cargo install --git https://github.com/stphnsmpsn/unifi-protect-backup-rs
```

### Option 3: Build from Source

```bash
# Clone the repository
git clone https://github.com/stphnsmpsn/unifi-protect-backup-rs.git
cd unifi-protect-backup-rs

# Build release version
cargo build --release

# Install to system
cargo install --path .
```

### Option 4: Docker (Coming Soon)

```bash
docker pull stphnsmpsn/unifi-protect-backup-rs:latest
```

## Verification

Verify the installation:

```bash
unifi-protect-backup-rs --version
```

## Initial Setup

### 1. Create Configuration Directory

```bash
mkdir -p ~/.unifi-protect-backup
```

### 2. Run Interactive Setup

```bash
unifi-protect-backup-rs
```

This will guide you through creating your initial configuration file.

### 3. Verify Configuration

```bash
unifi-protect-backup-rs --validate
```

## System Service Setup

### Linux (systemd)

Create a systemd service file:

```bash
sudo tee /etc/systemd/system/unifi-protect-backup.service > /dev/null <<EOF
[Unit]
Description=UniFi Protect Backup Service
After=network.target

[Service]
Type=simple
User=backup
Group=backup
WorkingDirectory=/opt/unifi-protect-backup
ExecStart=/usr/local/bin/unifi-protect-backup-rs
Restart=always
RestartSec=10
Environment=UFP_CONFIG_PATH=/opt/unifi-protect-backup/config.toml

[Install]
WantedBy=multi-user.target
EOF
```

Create backup user and directories:

```bash
sudo useradd -r -s /bin/false backup
sudo mkdir -p /opt/unifi-protect-backup
sudo chown backup:backup /opt/unifi-protect-backup
```

Enable and start the service:

```bash
sudo systemctl daemon-reload
sudo systemctl enable unifi-protect-backup
sudo systemctl start unifi-protect-backup
```

### macOS (launchd)

Create a launch daemon:

```bash
sudo tee /Library/LaunchDaemons/com.stphnsmpsn.unifi-protect-backup.plist > /dev/null <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.stphnsmpsn.unifi-protect-backup</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/unifi-protect-backup-rs</string>
    </array>
    <key>WorkingDirectory</key>
    <string>/usr/local/var/unifi-protect-backup</string>
    <key>KeepAlive</key>
    <true/>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
EOF
```

Load the service:

```bash
sudo launchctl load /Library/LaunchDaemons/com.stphnsmpsn.unifi-protect-backup.plist
```

## Configuration Paths

The application looks for configuration files in this order:

1. `--config` command line argument
2. `UFP_CONFIG` environment variable
3. `~/.unifi-protect-backup/config.toml`
4. `./config.toml` (current directory)

## Environment Variables

Set these environment variables for production deployments:

```bash
# Required
export UFP_CONFIG_PATH="/path/to/config.toml"

# Optional overrides
export UFP_UNIFI_ADDRESS="192.168.1.100"
export UFP_UNIFI_USERNAME="backup-user"
export UFP_UNIFI_PASSWORD="your-secure-password"
export UFP_DATABASE_PATH="/var/lib/unifi-protect-backup/events.db"
```

## Security Considerations

### File Permissions

```bash
# Restrict config file access
chmod 600 ~/.unifi-protect-backup/config.toml

# Secure backup directory
chmod 750 /path/to/backup/directory
```

### Network Security

- Use strong passwords for UniFi Protect user
- Consider VPN access for remote deployments
- Use SSH keys for Borg repositories

### Backup Security

- Enable encryption for Borg archives
- Use separate credentials for backup destinations
- Regularly test backup restoration

## Troubleshooting Installation

### Permission Issues

```bash
# Fix binary permissions
sudo chmod +x /usr/local/bin/unifi-protect-backup-rs

# Fix config directory permissions
sudo chown -R $USER:$USER ~/.unifi-protect-backup
```

### Missing Dependencies

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install borgbackup rclone openssh-client

# CentOS/RHEL
sudo yum install borgbackup rclone openssh-clients

# macOS
brew install borgbackup rclone
```

### Path Issues

Add to your shell profile (`~/.bashrc`, `~/.zshrc`):

```bash
export PATH="/usr/local/bin:$PATH"
```

## Next Steps

- [Configuration Reference](configuration.md) - Set up your first configuration
- [Quick Start](quickstart.md) - Get backing up quickly
- [Configuration Reference](configuration.md) - Detailed configuration options