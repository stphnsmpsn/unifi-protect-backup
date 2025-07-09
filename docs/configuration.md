# Configuration Reference

Complete reference for all configuration options in UniFi Protect Backup.

## Configuration File Structure

The application uses TOML format with the following main sections:

```toml
[unifi]        # UniFi Protect connection settings
[backup]       # Real-time backup configuration
[archive]      # Long-term archive configuration  
[database]     # Database settings
[notifications] # Email notifications (optional)
```

## UniFi Protect Connection

Configure connection to your UniFi Protect controller:

```toml
[unifi]
address = "192.168.1.100"     # Controller IP or hostname
port = 443                    # HTTPS port (default: 443)
username = "backup-user"      # UniFi user with viewing permissions
password = "your-password"    # User password
verify-ssl = false           # SSL certificate verification
```

### Security Options

Use environment variables or files for sensitive data:

```toml
[unifi]
address = "192.168.1.100"
username = "backup-user"
password = "env:UNIFI_PASSWORD"        # From environment variable
# password = "file:/path/to/password"   # From file
verify-ssl = true                      # Enable for production
```

## Backup Configuration

Real-time backup settings for immediate event storage:

```toml
[backup]
retention-period = "30d"              # How long to keep backups
poll-interval = "30s"                 # Database polling frequency
max-event-length = "5m"               # Maximum event duration
purge-interval = "24h"                # Cleanup frequency
file-structure-format = "{camera_name}/{date}/{time}_{detection_type}.mp4"
detection-types = ["motion", "person", "vehicle"]
ignore-cameras = []                   # Camera IDs to skip
cameras = []                          # Specific cameras (empty = all)
download-buffer-size = 8192           # Download buffer size in bytes
parallel-uploads = 3                  # Concurrent upload limit
skip-missing = false                  # Skip events with missing video
```

### Duration Format

All time-based fields support human-readable durations:

- `"30s"` - 30 seconds
- `"5m"` - 5 minutes  
- `"24h"` - 24 hours
- `"7d"` - 7 days
- `"4w"` - 4 weeks
- `"1y"` - 1 year

### File Structure Format

Customize backup file organization using template variables:

| Variable | Description | Example |
|----------|-------------|---------|
| `{camera_name}` | Camera display name | `"Front Door"` |
| `{camera_id}` | Camera unique ID | `"abc123def456"` |
| `{date}` | Event date | `"2024-01-15"` |
| `{time}` | Event time | `"14-30-25"` |
| `{end_time}` | Event end time | `"14-35-10"` |
| `{detection_type}` | Type of detection | `"motion"`, `"person"` |
| `{event_id}` | Unique event ID | `"event_abc123"` |

Example formats:
```toml
# Organized by camera and date
file-structure-format = "{camera_name}/{date}/{time}_{detection_type}.mp4"
# Result: "Front Door/2024-01-15/14-30-25_motion.mp4"

# Flat structure with full info
file-structure-format = "{date}_{time}_{camera_name}_{detection_type}.mp4"
# Result: "2024-01-15_14-30-25_Front Door_motion.mp4"
```

## Backup Targets

Configure where real-time backups are stored. Multiple targets are supported:

### Local Filesystem

```toml
[[backup.remote]]
local = { path-buf = "./data" }
```

### Rclone (Cloud Storage)

```toml
[[backup.remote]]
rclone = { remote = "s3:my-bucket", path = "/unifi-protect", config-file = "/path/to/rclone.conf" }
```

### Multiple Targets

```toml
# Local backup for fast access
[[backup.remote]]
local = { path-buf = "/mnt/fast-storage" }

# Cloud backup for redundancy
[[backup.remote]]
rclone = { remote = "s3:backup-bucket" }
```

## Archive Configuration

Long-term archive settings for encrypted, deduplicated storage:

```toml
[archive]
archive-interval = "1d"               # How often to create archives
retention-period = "365d"             # Archive retention period
file-structure-format = "{camera_name}/{date}/{time}_{detection_type}.mp4"
purge-interval = "1w"                 # Archive cleanup frequency
```

### Borg Archive Targets

```toml
[[archive.remote]]
borg = { borg-repo = "user@rsync.net:unifi-protect", borg-passphrase = "env:BORG_PASSPHRASE", ssh-key-path = "/home/user/.ssh/borg_key" }
```

### Multiple Archives

```toml
# Primary archive
[[archive.remote]]
borg = { borg-repo = "user@primary.backup.com:unifi", borg-passphrase = "env:BORG_PRIMARY_PASS" }

# Offsite archive
[[archive.remote]]
borg = { borg-repo = "user@offsite.backup.com:unifi", borg-passphrase = "env:BORG_OFFSITE_PASS", ssh-key-path = "/home/user/.ssh/offsite_key" }
```

## Database Configuration

SQLite database settings for event tracking:

```toml
[database]
path = "/var/lib/unifi-protect-backup/events.db"
```

The database automatically:
- Tracks all detected events
- Records backup status  
- Maintains foreign key relationships
- Handles concurrent access safely

## Notifications (Optional)

Email notifications for backup events:

```toml
[notifications]
smtp-host = "smtp.gmail.com"
smtp-port = 587
smtp-username = "your-email@gmail.com"
smtp-password = "env:SMTP_PASSWORD"
email-from = "backup@yourdomain.com"
email-to = "admin@yourdomain.com"
```

## Environment Variable Overrides

Any configuration value can be overridden with environment variables using the `UFP_` prefix:

```bash
# Override UniFi settings
export UFP_UNIFI_ADDRESS="10.0.1.100"
export UFP_UNIFI_USERNAME="admin"
export UFP_UNIFI_PASSWORD="secure-password"

# Override backup settings
export UFP_BACKUP_RETENTION_PERIOD="60d"
export UFP_BACKUP_POLL_INTERVAL="15s"

# Override database path
export UFP_DATABASE_PATH="/custom/path/events.db"
```

## Configuration Validation

Validate your configuration:

```bash
# Check configuration syntax and connectivity
unifi-protect-backup-rs --validate

# Test with specific config file
unifi-protect-backup-rs --config /path/to/config.toml --validate
```

## Example Configurations

### Minimal Home Setup

```toml
[unifi]
address = "192.168.1.100"
username = "backup"
password = "env:UNIFI_PASSWORD"
verify-ssl = false

[backup]
retention-period = "30d"
poll-interval = "30s"
detection-types = ["motion", "person"]

[[backup.remote]]
local = { path-buf = "./backups" }

[database]
path = "./events.db"
```

### Production Multi-Target Setup

```toml
[unifi]
address = "unifi.company.com"
username = "backup-service"
password = "env:UNIFI_PASSWORD"
verify-ssl = true

[backup]
retention-period = "30d"
poll-interval = "10s"
max-event-length = "10m"
detection-types = ["motion", "person", "vehicle", "package"]
parallel-uploads = 5
download-buffer-size = 16384

# Fast local storage
[[backup.remote]]
local = { path-buf = "/mnt/nvme/backups" }

# Cloud redundancy
[[backup.remote]]
rclone = { remote = "s3:company-backups", path = "/unifi-protect" }

[archive]
archive-interval = "6h"
retention-period = "7y"
purge-interval = "1d"

# Primary encrypted archive
[[archive.remote]]
borg = { borg-repo = "backup@archive.company.com:unifi", borg-passphrase = "env:BORG_PASSPHRASE", ssh-key-path = "/etc/unifi-backup/borg.key" }

# Offsite archive
[[archive.remote]]
borg = { borg-repo = "user@rsync.net:company-unifi", borg-passphrase = "env:BORG_OFFSITE_PASS" }

[database]
path = "/var/lib/unifi-protect-backup/events.db"

[notifications]
smtp-host = "smtp.company.com"
smtp-port = 587
smtp-username = "backup-service@company.com"
smtp-password = "env:SMTP_PASSWORD"
email-from = "backup-service@company.com"
email-to = "admin@company.com"
```

## Security Best Practices

### Credentials Management

1. **Never store passwords in config files**:
   ```toml
   password = "env:UNIFI_PASSWORD"  # ✅ Good
   password = "my-password"         # ❌ Bad
   ```

2. **Use SSH keys for Borg repositories**:
   ```toml
   ssh-key-path = "/path/to/dedicated/key"
   ```

3. **Restrict file permissions**:
   ```bash
   chmod 600 config.toml
   chmod 700 ~/.unifi-protect-backup/
   ```

### Network Security

1. **Enable SSL verification** in production
2. **Use dedicated backup user** with minimal permissions
3. **Consider VPN** for remote access

## Troubleshooting Configuration

### Common Issues

1. **Connection refused**: Check UniFi address and port
2. **Authentication failed**: Verify username/password
3. **Permission denied**: Check file/directory permissions
4. **Invalid duration**: Use format like `"30d"`, `"24h"`, `"5m"`

### Debug Mode

Enable verbose logging:

```bash
RUST_LOG=debug unifi-protect-backup-rs
```

### Configuration Testing

```bash
# Test UniFi connection
unifi-protect-backup-rs --test-connection

# Validate all settings
unifi-protect-backup-rs --validate

# Dry run without actual backups
unifi-protect-backup-rs --dry-run
```