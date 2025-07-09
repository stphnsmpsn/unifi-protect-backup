# Quick Start Guide

Get up and running with UniFi Protect Backup in minutes.

## Prerequisites

Before starting, ensure you have:

- ✅ UniFi Protect controller running and accessible
- ✅ User account with viewing permissions on UniFi Protect
- ✅ Network access from backup server to UniFi Protect
- ✅ Sufficient storage space for backups

## Step 1: Install the Application

Choose your preferred installation method:

=== "Pre-built Binary"
    ```bash
    # Download latest release
    curl -LO https://github.com/stphnsmpsn/unifi-protect-backup-rs/releases/latest/download/unifi-protect-backup-rs-linux-x86_64.tar.gz
    tar -xzf unifi-protect-backup-rs-linux-x86_64.tar.gz
    sudo mv unifi-protect-backup-rs /usr/local/bin/
    sudo chmod +x /usr/local/bin/unifi-protect-backup-rs
    ```

=== "Cargo Install"
    ```bash
    cargo install --git https://github.com/stphnsmpsn/unifi-protect-backup-rs
    ```

=== "From Source"
    ```bash
    git clone https://github.com/stphnsmpsn/unifi-protect-backup-rs.git
    cd unifi-protect-backup-rs
    cargo build --release
    cargo install --path .
    ```

Verify installation:
```bash
unifi-protect-backup-rs --version
```

## Step 2: Interactive Configuration

Run the interactive setup wizard:

```bash
unifi-protect-backup-rs
```

The wizard will prompt you for:

### UniFi Protect Settings
```
UniFi Protect address [192.168.1.100]: 10.0.1.100
Port [443]: 443
Username [backup-user]: backup
Password [your-password]: mypassword
Verify SSL (true/false) [false]: false
```

### Backup Configuration
```
Backup targets (comma-separated) [1]: 1
Local backup path [./data]: /home/user/unifi-backups

Backup retention period (e.g., 30d, 1w) [30d]: 30d
Poll interval (e.g., 30s, 1m) [30s]: 30s
Detection types (comma-separated) [motion,person,vehicle]: motion,person
Max event length (e.g., 5m, 300s) [5m]: 5m
```

### Archive Configuration
```
Archive interval (e.g., 1h, 1d, 1w) [1d]: 1d
Archive retention period (e.g., 30d, 1y) [365d]: 90d
Archive targets (comma-separated) [1]: 1

Borg repository [user@rsync.net:unifi-protect]: user@myserver.com:backups
SSH key path (optional): /home/user/.ssh/backup_key
Borg passphrase (optional): mypassphrase
```

This creates `~/.unifi-protect-backup/config.toml`.

## Step 3: Test Configuration

Validate your configuration:

```bash
unifi-protect-backup-rs --validate
```

Expected output:
```
✅ Configuration file is valid
✅ UniFi Protect connection successful
✅ Database accessible
✅ Backup targets configured
✅ Archive targets configured
```

## Step 4: Run First Backup

Start the application:

```bash
unifi-protect-backup-rs
```

You should see output like:
```
2024-01-15T10:30:00Z INFO Starting UniFi Protect Backup
2024-01-15T10:30:01Z INFO Connected to UniFi Protect at 10.0.1.100
2024-01-15T10:30:01Z INFO Found 3 cameras: Front Door, Back Yard, Living Room
2024-01-15T10:30:01Z INFO Starting database poller
2024-01-15T10:30:01Z INFO Starting WebSocket event monitor
2024-01-15T10:30:02Z INFO WebSocket connected and listening for events
```

## Step 5: Verify Backup Operation

### Trigger a Test Event

1. Walk in front of a camera to trigger motion detection
2. Check the application logs for processing:

```
2024-01-15T10:35:15Z INFO Motion detected on Front Door camera
2024-01-15T10:35:15Z INFO Processing event abc123def456
2024-01-15T10:35:16Z INFO Downloaded 2.3MB video for event abc123def456
2024-01-15T10:35:17Z INFO Backed up to local storage: Front Door/2024-01-15/10-35-15_motion.mp4
```

### Check Backup Files

```bash
ls -la /home/user/unifi-backups/
```

You should see a directory structure like:
```
Front Door/
├── 2024-01-15/
│   └── 10-35-15_motion.mp4
Back Yard/
└── 2024-01-15/
    └── 09-22-30_motion.mp4
```

## Step 6: Monitor Operation

### Check Database Status

```bash
# View recent events
sqlite3 ~/.unifi-protect-backup/events.db "
SELECT camera_name, detection_type, backed_up, datetime(start_time, 'unixepoch') 
FROM events 
ORDER BY start_time DESC 
LIMIT 10;"
```

### Monitor Logs

For detailed logging:
```bash
RUST_LOG=info unifi-protect-backup-rs
```

For debug logging:
```bash
RUST_LOG=debug unifi-protect-backup-rs
```

## Common Quick Start Issues

### Connection Issues

**Problem**: "Connection refused to UniFi Protect"
```bash
# Test connectivity
ping 10.0.1.100
curl -k https://10.0.1.100
```

**Problem**: "Authentication failed"
- Verify username and password
- Ensure user has viewer permissions
- Check if 2FA is enabled (not supported)

### Permission Issues

**Problem**: "Permission denied" for backup directory
```bash
# Fix permissions
sudo chown -R $USER:$USER /home/user/unifi-backups
chmod 755 /home/user/unifi-backups
```

**Problem**: "Config file not readable"
```bash
# Fix config permissions
chmod 600 ~/.unifi-protect-backup/config.toml
```

### Storage Issues

**Problem**: "No space left on device"
- Check available disk space: `df -h`
- Adjust retention periods in config
- Set up automatic cleanup

## Next Steps

### Production Deployment

1. **Set up as a system service**: [Installation Guide](installation.md#system-service-setup)
2. **Configure advanced options**: [Configuration Reference](configuration.md)
3. **Set up notifications**: [Configuration Reference](configuration.md#notifications-optional)

### Advanced Configuration

1. **Multiple backup targets**: [Configuration Reference](configuration.md#backup-targets)
2. **Archive optimization**: [Configuration Reference](configuration.md#archive-configuration)
3. **Security hardening**: [Configuration Reference](configuration.md#security-best-practices)

### Customization

1. **File organization**: [Configuration Reference](configuration.md#file-structure-format)
2. **Event filtering**: [Configuration Reference](configuration.md#backup-configuration)
3. **Performance tuning**: [Architecture Overview](architecture.md#performance-characteristics)

## Example: Complete Home Setup

Here's a complete example for a typical home installation:

### 1. Directory Setup
```bash
mkdir -p /home/user/unifi-protect/{backups,config,logs}
```

### 2. Configuration File
```toml
# /home/user/unifi-protect/config/config.toml
[unifi]
address = "192.168.1.100"
username = "backup"
password = "env:UNIFI_PASSWORD"
verify-ssl = false

[backup]
retention-period = "30d"
poll-interval = "30s"
max-event-length = "5m"
detection-types = ["motion", "person"]
file-structure-format = "{camera_name}/{date}/{time}_{detection_type}.mp4"

[[backup.remote]]
local = { path-buf = "/home/user/unifi-protect/backups" }

[archive]
archive-interval = "1d"
retention-period = "90d"

[[archive.remote]]
borg = { borg-repo = "backup@nas.local:unifi", borg-passphrase = "env:BORG_PASSPHRASE" }

[database]
path = "/home/user/unifi-protect/config/events.db"
```

### 3. Environment Variables
```bash
# Add to ~/.bashrc or ~/.profile
export UNIFI_PASSWORD="your-secure-password"
export BORG_PASSPHRASE="your-borg-passphrase"
```

### 4. Start Script
```bash
#!/bin/bash
# /home/user/unifi-protect/start.sh
cd /home/user/unifi-protect
export RUST_LOG=info
exec unifi-protect-backup-rs --config config/config.toml >> logs/backup.log 2>&1
```

### 5. Run
```bash
chmod +x /home/user/unifi-protect/start.sh
/home/user/unifi-protect/start.sh
```

This setup provides:
- ✅ Local backup to filesystem
- ✅ Daily encrypted archives
- ✅ 30-day backup retention
- ✅ 90-day archive retention
- ✅ Motion and person detection
- ✅ Organized file structure
- ✅ Secure credential management