# Usage Guide

Comprehensive guide for running and operating UniFi Protect Backup.

## Command Line Interface

UniFi Protect Backup follows a **config-only approach** with minimal command line options:

```bash
# Basic usage (uses default config location)
unifi-protect-backup-rs

# Specify custom config file
unifi-protect-backup-rs --config /path/to/config.toml

# Validate configuration without running
unifi-protect-backup-rs --validate

# Show version information
unifi-protect-backup-rs --version

# Show help
unifi-protect-backup-rs --help
```

## Configuration File Locations

The application searches for configuration files in this order:

1. `--config` command line argument
2. `UFP_CONFIG` environment variable
3. `~/.unifi-protect-backup/config.toml`
4. `./config.toml` (current directory)

```bash
# Example: Override config location
export UFP_CONFIG="/etc/unifi-protect-backup/config.toml"
unifi-protect-backup-rs
```

## Running Modes

### Interactive Setup Mode

When no configuration exists, the application launches an interactive setup wizard:

```bash
unifi-protect-backup-rs
# Launches setup wizard if no config found
```

### Normal Operation Mode

With existing configuration, the application runs continuously:

```bash
unifi-protect-backup-rs --config /path/to/config.toml
```

Expected startup sequence:
```
INFO  Starting UniFi Protect Backup v1.0.0
INFO  Loading configuration from: /path/to/config.toml
INFO  Connecting to UniFi Protect at 192.168.1.100
INFO  Connected successfully, found 5 cameras
INFO  Database initialized: /var/lib/unifi-backup/events.db
INFO  Starting WebSocket event monitor
INFO  Starting database poller (poll-interval: 30s)
INFO  Application ready, monitoring for events
```

### Validation Mode

Test configuration without running the backup service:

```bash
unifi-protect-backup-rs --validate
```

Validation checks:
- ✅ Configuration file syntax
- ✅ UniFi Protect connectivity
- ✅ Database accessibility
- ✅ Backup target availability
- ✅ Archive target connectivity
- ✅ Required dependencies (borg, rclone)

## Environment Variables

### Configuration Overrides

Any configuration value can be overridden with environment variables:

```bash
# UniFi Protect settings
export UFP_UNIFI_ADDRESS="10.0.1.100"
export UFP_UNIFI_USERNAME="backup-user"
export UFP_UNIFI_PASSWORD="secure-password"
export UFP_UNIFI_VERIFY_SSL="true"

# Backup settings
export UFP_BACKUP_RETENTION_PERIOD="60d"
export UFP_BACKUP_POLL_INTERVAL="15s"

# Database settings
export UFP_DATABASE_PATH="/custom/path/events.db"
```

### Logging Configuration

Control logging verbosity:

```bash
# Info level logging (default)
export RUST_LOG=info

# Debug level logging
export RUST_LOG=debug

# Module-specific logging
export RUST_LOG="unifi_protect_backup=debug,sqlx=warn"

# JSON structured logging
export UFP_LOG_FORMAT=json
```

### Runtime Behavior

```bash
# Disable interactive prompts
export UFP_NON_INTERACTIVE=true

# Override config file location
export UFP_CONFIG="/etc/unifi-protect-backup/config.toml"

# Enable performance metrics
export UFP_METRICS_ENABLED=true
```

## Operation Monitoring

### Log Analysis

Monitor application logs for key events:

```bash
# Follow logs in real-time
tail -f /var/log/unifi-protect-backup.log

# Search for specific events
grep "Processing event" /var/log/unifi-protect-backup.log
grep "ERROR" /var/log/unifi-protect-backup.log
```

Key log patterns to monitor:

```bash
# Successful event processing
"INFO.*Processing event.*camera_name.*detection_type"
"INFO.*Backed up motion event.*filename"

# Connection issues
"ERROR.*Failed to connect to UniFi Protect"
"WARN.*WebSocket connection lost, reconnecting"

# Storage issues
"ERROR.*Backup failed.*No space left"
"ERROR.*Archive creation failed"
```

### Database Monitoring

Query the SQLite database for operational insights:

```bash
# Connect to database
sqlite3 ~/.unifi-protect-backup/events.db

# Recent event summary
.mode column
.headers on
SELECT 
    camera_name,
    detection_type,
    COUNT(*) as events,
    SUM(CASE WHEN backed_up THEN 1 ELSE 0 END) as backed_up,
    MAX(datetime(start_time, 'unixepoch')) as latest_event
FROM events 
WHERE start_time > strftime('%s', 'now', '-24 hours')
GROUP BY camera_name, detection_type;
```

```bash
# Backup success rate
SELECT 
    DATE(start_time, 'unixepoch') as date,
    COUNT(*) as total_events,
    SUM(CASE WHEN backed_up THEN 1 ELSE 0 END) as successful_backups,
    ROUND(100.0 * SUM(CASE WHEN backed_up THEN 1 ELSE 0 END) / COUNT(*), 2) as success_rate
FROM events 
WHERE start_time > strftime('%s', 'now', '-7 days')
GROUP BY DATE(start_time, 'unixepoch')
ORDER BY date DESC;
```

```bash
# Storage usage by camera
SELECT 
    b.target_name,
    e.camera_name,
    COUNT(b.id) as backup_count,
    ROUND(SUM(b.file_size) / 1024.0 / 1024.0, 2) as size_mb
FROM backups b
JOIN events e ON b.event_id = e.id
WHERE b.created_at > strftime('%s', 'now', '-30 days')
GROUP BY b.target_name, e.camera_name
ORDER BY size_mb DESC;
```

### System Resource Monitoring

Monitor system resources:

```bash
# Memory usage
ps aux | grep unifi-protect-backup-rs

# Disk usage
df -h /path/to/backup/directory

# Network connections
netstat -tulpn | grep unifi-protect-backup-rs

# File descriptor usage
lsof -p $(pgrep unifi-protect-backup-rs)
```

## Backup Management

### Manual Backup Operations

Force backup of specific events:

```bash
# Trigger backup of unbacked events
sqlite3 ~/.unifi-protect-backup/events.db "
UPDATE events 
SET backed_up = FALSE 
WHERE id IN ('event1', 'event2');"
```

### Backup Verification

Verify backup integrity:

```bash
# Check backup file existence
find /backup/path -name "*.mp4" -type f | wc -l

# Compare with database records
sqlite3 ~/.unifi-protect-backup/events.db "
SELECT COUNT(*) FROM events WHERE backed_up = TRUE;"

# Check for missing backups
sqlite3 ~/.unifi-protect-backup/events.db "
SELECT id, camera_name, datetime(start_time, 'unixepoch') 
FROM events 
WHERE backed_up = FALSE 
ORDER BY start_time DESC 
LIMIT 10;"
```

### Archive Operations

Manual archive creation:

```bash
# Trigger archive creation (if using Borg)
borg create /path/to/repo::backup-$(date +%Y%m%d-%H%M%S) /backup/path

# List archives
borg list /path/to/repo

# Check archive integrity
borg check /path/to/repo
```

## Performance Optimization

### Configuration Tuning

Optimize for your environment:

```toml
[backup]
# High-throughput environment
poll-interval = "10s"
parallel-uploads = 5
download-buffer-size = 16384

# Resource-constrained environment
poll-interval = "60s"
parallel-uploads = 2
download-buffer-size = 4096
```

### System Tuning

Operating system optimizations:

```bash
# Increase file descriptor limits
echo "fs.file-max = 65536" >> /etc/sysctl.conf
echo "unifi-backup soft nofile 65536" >> /etc/security/limits.conf
echo "unifi-backup hard nofile 65536" >> /etc/security/limits.conf

# Optimize for high I/O
echo 'mq-deadline' > /sys/block/sda/queue/scheduler
echo 8192 > /sys/block/sda/queue/read_ahead_kb
```

### Storage Optimization

Optimize storage performance:

```bash
# Use faster storage for database
mount -t tmpfs -o size=100M tmpfs /var/lib/unifi-backup/db

# Enable compression for backup storage
mount -o compress=zstd /dev/sdb1 /backup/storage
```

## Troubleshooting

### Common Issues

#### High CPU Usage
```bash
# Check if too many parallel operations
grep "parallel-uploads\|poll-interval" config.toml

# Reduce concurrency
poll-interval = "60s"
parallel-uploads = 2
```

#### Memory Usage Growing
```bash
# Check for memory leaks
valgrind --tool=memcheck unifi-protect-backup-rs

# Restart service periodically (workaround)
systemctl restart unifi-protect-backup
```

#### Disk Space Issues
```bash
# Check retention settings
grep "retention-period" config.toml

# Manual cleanup
find /backup/path -name "*.mp4" -mtime +30 -delete

# Archive old backups
borg create /path/to/archive::old-backups-$(date +%Y%m%d) /backup/path
```

### Debug Mode

Enable comprehensive debugging:

```bash
RUST_LOG=debug,sqlx=debug unifi-protect-backup-rs 2>&1 | tee debug.log
```

Debug output includes:
- Detailed connection attempts
- WebSocket message content
- Database query execution
- File system operations
- Network request/response details

### Recovery Procedures

#### Database Corruption
```bash
# Backup corrupted database
cp ~/.unifi-protect-backup/events.db ~/.unifi-protect-backup/events.db.corrupt

# Attempt repair
sqlite3 ~/.unifi-protect-backup/events.db ".recover" | sqlite3 recovered.db

# If repair fails, rebuild from backups
# (Application will recreate schema on startup)
rm ~/.unifi-protect-backup/events.db
```

#### Configuration Issues
```bash
# Reset to defaults
mv ~/.unifi-protect-backup/config.toml ~/.unifi-protect-backup/config.toml.backup
unifi-protect-backup-rs  # Runs setup wizard
```

#### Connection Recovery
```bash
# Test network connectivity
ping unifi-controller-ip
curl -k https://unifi-controller-ip

# Check certificate issues
openssl s_client -connect unifi-controller-ip:443 -verify_return_error

# Reset WebSocket connection
systemctl restart unifi-protect-backup
```

## Integration Examples

### Systemd Service Management

```bash
# Check service status
systemctl status unifi-protect-backup

# View service logs
journalctl -u unifi-protect-backup -f

# Restart service
systemctl restart unifi-protect-backup

# Enable auto-start
systemctl enable unifi-protect-backup
```

### Monitoring Integration

#### Prometheus Metrics (Future Feature)
```bash
# Metrics endpoint
curl http://localhost:9090/metrics
```

#### Log Aggregation
```bash
# Rsyslog configuration
echo "*.* @@logserver:514" >> /etc/rsyslog.conf
systemctl restart rsyslog
```

### Backup Integration

#### External Backup Tools
```bash
# Include in existing backup jobs
rsync -av /backup/path/ backup-server:/unifi-backups/

# Cloud sync
rclone sync /backup/path/ remote:unifi-backups
```

## Best Practices

### Operational Best Practices

1. **Monitor disk space**: Set up alerts for storage usage
2. **Test restores**: Regularly verify backup integrity
3. **Update regularly**: Keep application and dependencies updated
4. **Secure credentials**: Use environment variables for passwords
5. **Log rotation**: Configure log rotation to prevent disk filling

### Performance Best Practices

1. **Use SSD storage**: For database and temporary files
2. **Tune concurrency**: Based on available resources
3. **Monitor resources**: CPU, memory, disk I/O, network
4. **Archive regularly**: Move old backups to cold storage
5. **Network optimization**: Ensure adequate bandwidth

### Security Best Practices

1. **Dedicated user**: Run service as non-root user
2. **File permissions**: Restrict access to config and data files
3. **Network security**: Use VPN for remote access
4. **Encryption**: Enable encryption for archives
5. **Audit logs**: Monitor access and operations