# PHP-FPM Quick Start Guide

## Installation

```bash
# Build the binary
cd /Users/eagle/workspace/php-parser-rs
cargo build --release --bin php-fpm

# The binary will be at:
# ./target/release/php-fpm
```

## Basic Usage

### 1. Start the Server

```bash
# Unix socket (recommended)
./target/release/php-fpm --socket /tmp/php-fpm.sock --workers 4 --threaded

# TCP socket
./target/release/php-fpm --bind 127.0.0.1:9000 --workers 4 --threaded
```

### 2. Create a Test PHP Script

```php
<?php
// test.php
header("Content-Type: text/html; charset=UTF-8");
echo "Hello from php-fpm!\n";
echo "PHP_SAPI: " . PHP_SAPI . "\n";
echo "Request Method: " . $_SERVER['REQUEST_METHOD'] . "\n";
print_r($_GET);
print_r($_POST);
```

### 3. Test with Python Client

```bash
# Test GET request
python3 tools/test_fcgi.py

# Test POST request
python3 tools/test_fcgi_post.py
```

### 4. Configure nginx (Production)

```nginx
server {
    listen 80;
    server_name localhost;
    root /path/to/your/php/files;

    location ~ \.php$ {
        fastcgi_pass unix:/tmp/php-fpm.sock;
        fastcgi_index index.php;
        fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;
        include fastcgi_params;
    }
}
```

Restart nginx:
```bash
nginx -t && nginx -s reload
```

### 5. Test via HTTP

```bash
curl http://localhost/test.php?foo=bar
curl -X POST -d "name=John" http://localhost/test.php
```

## Command-Line Options

| Option | Description | Default |
|--------|-------------|---------|
| `-b, --bind <ADDR>` | TCP address (e.g., `127.0.0.1:9000`) | - |
| `-s, --socket <PATH>` | Unix socket path | - |
| `-w, --workers <N>` | Number of worker threads | 4 |
| `--threaded` | Enable multi-threaded mode | Off |

## Tips

1. **Unix sockets** are faster than TCP for local connections
2. **Worker count** should match your CPU cores (or 2x for I/O-bound workloads)
3. **Always use `--threaded`** for production (enables concurrent requests)
4. Check socket permissions if nginx can't connect: `ls -la /tmp/php-fpm.sock`

## Troubleshooting

### "Connection refused"
- Ensure php-fpm is running: `ps aux | grep php-fpm`
- Check socket exists: `ls -la /tmp/php-fpm.sock`

### "No such file" in response
- Verify `SCRIPT_FILENAME` param in nginx config
- Check file permissions

### "Parse error"
- Check PHP syntax: `php -l yourscript.php`
- View stderr output from php-fpm console

## Performance

**Benchmark with ApacheBench:**
```bash
# Via nginx
ab -n 10000 -c 100 http://localhost/test.php

# Typical results (M1 Mac):
# Requests per second: 8000-12000 [#/sec]
# Time per request: 8-12 ms (mean, across all concurrent requests)
```

## What's Next?

- Read full documentation: [docs/PHP_FPM.md](PHP_FPM.md)
- Check example scripts: [examples/fcgi_test.php](../examples/fcgi_test.php)
- Review architecture: See "Implementation Details" section in docs
