# PHP-FPM Implementation

## Overview

This is a FastCGI Process Manager (php-fpm) implementation for the Rust PHP VM. It provides both single-threaded and multi-threaded modes for serving PHP scripts via the FastCGI protocol.

## Features

- ✅ **FastCGI Protocol 1.0** - Full implementation with record parsing/encoding
- ✅ **Multi-threaded Workers** - Concurrent request handling with per-thread engines
- ✅ **Unix Socket & TCP Support** - Listen on Unix sockets or TCP ports
- ✅ **Superglobal Population** - Automatic $_SERVER, $_GET, $_POST, $_ENV setup
- ✅ **Header Management** - HTTP status codes and headers from `header()` function
- ✅ **Graceful Shutdown** - CTRL-C signal handling
- ✅ **Zero-panic Protocol** - Malformed FastCGI frames return errors, never panic

## Architecture

### Concurrency Model

**Per-Thread Engine (Current Implementation)**
- Each worker thread owns its own `Engine` instance
- Fresh `VM` + `RequestContext` created per request
- No cross-thread sharing (thread-safe by isolation)
- Semantics: MINIT happens once per thread, RINIT/RSHUTDOWN per request

**Why Not Shared Engine?**
The VM uses `Rc`/`RefCell` (not `Send`/`Sync`) for values and resources. Making the engine thread-shareable would require:
1. Changing `Val` from `Rc<T>` to `Arc<T>`
2. Refactoring engine-global storage (constants, module globals)
3. Ensuring all resource types (MySQLi, PDO, Zip, etc.) are `Send`

This is planned for future versions. For now, prefork (process-per-worker) remains the most PHP-compatible mode.

### Request Lifecycle

```
1. Accept FastCGI connection (TCP or Unix socket)
2. Read BEGIN_REQUEST + PARAMS + STDIN records
3. Parse params into $_SERVER, $_GET, $_POST, etc.
4. Load and compile PHP script (SCRIPT_FILENAME)
5. Execute in fresh VM with captured output
6. Serialize headers + body to STDOUT record
7. Send END_REQUEST with exit code
8. Loop if keep-alive, else close connection
```

## Usage

### Build

```bash
cargo build --release --bin php-fpm
```

### Run

**Single worker (for development/debugging):**
```bash
./target/release/php-fpm --socket /tmp/php-fpm.sock --workers 1
```

**Multi-threaded mode (4 workers, recommended):**
```bash
./target/release/php-fpm --socket /tmp/php-fpm.sock --workers 4
```

**TCP mode:**
```bash
./target/release/php-fpm --bind 127.0.0.1:9000 --workers 8
```

### Configure nginx

```nginx
location ~ \.php$ {
    fastcgi_pass unix:/tmp/php-fpm.sock;
    fastcgi_index index.php;
    fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;
    include fastcgi_params;
}
```

### Test with cgi-fcgi

```bash
# Install fcgi tools
# macOS: brew install fcgi
# Debian: apt-get install libfcgi-dev

# Send request
SCRIPT_FILENAME=/path/to/script.php \
REQUEST_METHOD=GET \
cgi-fcgi -bind -connect /tmp/php-fpm.sock
```

## Command-Line Options

```
php-fpm [OPTIONS]

OPTIONS:
    -b, --bind <BIND>        Listen on TCP (e.g., "127.0.0.1:9000") [conflicts with --socket]
    -s, --socket <SOCKET>    Listen on Unix socket (e.g., "/tmp/php-fpm.sock") [conflicts with --bind]
    -w, --workers <WORKERS>  Number of worker threads [default: 4]
    -h, --help              Print help
    -V, --version           Print version
```

Options:
  -b, --bind <ADDR>         Listen on TCP (e.g., "127.0.0.1:9000")
  -s, --socket <PATH>       Listen on Unix socket (e.g., "/tmp/php-fpm.sock")
  -w, --workers <N>         Number of worker threads (default: 4)
      --threaded            Enable multi-threaded mode
  -h, --help                Print help
```

## Implementation Details

### Modules

- **`fcgi/protocol.rs`** - FastCGI record parsing/encoding, no-panic on malformed input
- **`fcgi/request.rs`** - Request accumulator (BEGIN_REQUEST → PARAMS → STDIN)
- **`sapi/fpm.rs`** - FastCGI → superglobal mapping (QUERY_STRING → $_GET, etc.)
- **`sapi/mod.rs`** - Superglobal initialization helpers
- **`bin/php-fpm.rs`** - Server entrypoint, worker threads, connection handling

### Superglobal Mapping

| FastCGI Param        | PHP Superglobal  | Notes                                    |
|----------------------|------------------|------------------------------------------|
| All params           | `$_SERVER`       | Full CGI environment                     |
| `QUERY_STRING`       | `$_GET`          | URL-decoded key=value pairs              |
| `STDIN` (POST body)  | `$_POST`         | Only for `application/x-www-form-urlencoded` |
| All params           | `$_ENV`          | Duplicate of `$_SERVER` for now          |
| Merged GET+POST      | `$_REQUEST`      | Standard PHP behavior                    |
| Empty                | `$_COOKIE`       | TODO: parse `HTTP_COOKIE` header         |
| Empty                | `$_FILES`        | TODO: multipart/form-data parsing        |

### Output Capturing

Currently using a custom `BufferedOutputWriter` that captures all `echo`/`print` output. Headers from `header()` function are stored in `RequestContext::headers` and serialized before the body.

### Error Handling

- **Parse errors**: Return 500 with error details in STDERR
- **Missing script**: Return 500 with "Failed to read script" in STDERR
- **Runtime errors**: Return 500 with `VmError` details in STDERR
- **Malformed FastCGI**: Log error, close connection cleanly (no panic)

## Testing

### Unit Tests

```bash
cargo test --lib fcgi
cargo test --lib sapi::fpm
```

### Integration Test

```bash
# Terminal 1: Start server
./target/release/php-fpm --socket /tmp/test.sock --workers 2 --threaded

# Terminal 2: Use Python test client
python3 tools/test_fcgi.py        # Test GET
python3 tools/test_fcgi_post.py   # Test POST

# Or use cgi-fcgi tool
SCRIPT_FILENAME=/path/to/script.php REQUEST_METHOD=GET \
  cgi-fcgi -bind -connect /tmp/test.sock
```

**Example Output:**
```
Status: 200 OK
Content-Type: text/html; charset=UTF-8

Hello from php-fpm!
PHP_SAPI: fpm-fcgi
_SERVER[REQUEST_METHOD]: GET
_GET: Array
(
    [foo] => bar
    [test] => 123
)
```

### Stress Test

```bash
# Using ab (ApacheBench) via nginx proxy
ab -n 10000 -c 100 http://localhost/test.php
```

## Limitations & TODOs

- [x] ~~Output capturing is placeholder~~ **DONE: Fully working with Arc<Mutex<Vec<u8>>>**
- [x] ~~GET and POST requests~~ **DONE: Verified working with test clients**
- [ ] `$_FILES` / multipart form parsing not implemented
- [ ] `$_COOKIE` parsing from `HTTP_COOKIE` header not implemented
- [ ] No prefork master/worker process manager (only threaded)
- [ ] No php-fpm.conf parsing (all config via CLI args)
- [ ] No process pool management (pm=dynamic, pm.max_children, etc.)
- [ ] No slow log, request timeouts, or resource limits

## Verified Working Features ✅

**GET Requests:**
- ✅ Query string parsing with URL decode (`foo=bar&test=123` → `$_GET['foo']`)
- ✅ `$_SERVER['QUERY_STRING']` correctly set
- ✅ Multiple parameters handled

**POST Requests:**
- ✅ `application/x-www-form-urlencoded` body parsing
- ✅ `$_POST` array populated correctly
- ✅ `$_SERVER['REQUEST_METHOD']` = `POST`
- ✅ `CONTENT_LENGTH` and `CONTENT_TYPE` params

**Superglobals:**
- ✅ `$_SERVER` - All FastCGI params mapped
- ✅ `$_GET` - Query string with URL decode
- ✅ `$_POST` - Form data parsing
- ✅ `$_ENV` - Environment variables
- ✅ `$_REQUEST` - Merged GET + POST
- ✅ `$_COOKIE` - Empty array (parsing TODO)
- ✅ `$_FILES` - Empty array (multipart TODO)
- ✅ `$GLOBALS` - Global scope access
- ✅ `PHP_SAPI` - Set to `"fpm-fcgi"`

**Headers & Output:**
- ✅ `header()` function stores headers
- ✅ HTTP status codes serialized to FastCGI
- ✅ Content-Type and custom headers
- ✅ `echo` / `print` captured to STDOUT
- ✅ Binary output support

## Future Enhancements

1. **Shared Engine Mode** - Refactor `Val` to `Arc` for true thread-pool concurrency
2. **Prefork Manager** - Master process + worker pool for better PHP semantics
3. **Configuration File** - Parse php-fpm.conf for pools, logging, limits
4. **Advanced Features** - Slow log, chroot, chdir, process title, systemd integration
5. **Performance** - Zero-copy `php://input`, sendfile, HTTP/2 FCGI

## Compatibility

- ✅ Nginx FastCGI
- ✅ Apache mod_fcgid
- ✅ Caddy FastCGI
- ⚠️ PHP-FPM config files (not supported)
- ⚠️ Unix socket permissions (no chmod/chown yet)

## References

- FastCGI Specification: https://fastcgi-archives.github.io/FastCGI_Specification.html
- PHP FastCGI: https://www.php.net/manual/en/install.fpm.php
- Zend Engine: `/Users/eagle/Sourcecode/php-src/Zend/zend_language_scanner.l`
