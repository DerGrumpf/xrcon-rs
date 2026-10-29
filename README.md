# xrcon

rcon client for Xonotic (DarkPlaces rcon protocol): a command line tool, a
Rust library (blocking and async) and Python bindings.

## Command line tool

Build it:

```
cargo build --release
```

The binary is `target/release/xrcon`.

### Usage

```
xrcon [OPTIONS] <COMMAND>...
```

| Option | Meaning |
|---|---|
| `-s`, `--server <SERVER>` | Server as `host:port` |
| `-p`, `--password <PASSWORD>` | rcon password |
| `-t`, `--type <{0,1,2}>` | Connection type: 0 = non-secure, 1 = time-secure (default), 2 = challenge-secure |
| `--timeout <TIMEOUT>` | Seconds to collect the reply |
| `-n`, `--name <NAME>` | Section of the config file to use |
| `--config <CONFIG>` | Config file path |

Example:

```
xrcon -s example.com:26000 -p 'secret' -t 1 status
```

### Config file

`xrcon` reads `~/.xrcon.ini` (or the file given with `--config`). Sections are
selected with `-n/--name`; `[DEFAULT]` is the fallback. Keys: `server`,
`password`, `type`, `timeout`.

Example:

```ini
[DEFAULT]
type = 1

[myserver]
server = example.com:26000
password = secret
```

```
xrcon -n myserver status
```

**Known limitation:** the INI parser treats `;` and `#` as comment markers,
even inside a value. A password containing `;` or `#` is cut off at that
character. For such passwords, use `-p` or the interactive prompt instead of
the config file.

## Rust library

The crate `xrcon` is split into a sans-IO core and two ready-made clients:

| Module | What it is |
|---|---|
| `xrcon::core` | `Session`: protocol logic with no sockets or clock |
| `xrcon::net` | `Client`: blocking client on `std::net::UdpSocket` |
| `xrcon::tokio` | `Client`: async client, behind the `tokio` feature |

### Blocking client

```rust
use xrcon::core::{Mode, PingKind};
use xrcon::net::Client;

fn main() {
    let mut client = Client::connect("example.com", 26000, "secret", Mode::Time)
        .expect("connect failed");

    // Round trip time in seconds, or None on timeout.
    if let Some(rtt) = client.ping(PingKind::Q2, 1.0).expect("ping failed") {
        println!("ping: {:.1} ms", rtt * 1000.0);
    }

    // Run a command and collect the reply for 0.7 seconds.
    let reply = client.execute(b"status", 0.7).expect("execute failed");
    println!("{}", String::from_utf8_lossy(&reply));
}
```

### Async client

Enable the `tokio` feature in your `Cargo.toml`:

```toml
xrcon = { version = "1.0", features = ["tokio"] }
```

```rust
use xrcon::core::{Mode, PingKind};
use xrcon::tokio::Client;

#[tokio::main]
async fn main() {
    let mut client = Client::connect("example.com", 26000, "secret", Mode::Time)
        .await
        .expect("connect failed");

    let rtt = client.ping(PingKind::Q2, 1.0).await.expect("ping failed");
    println!("ping: {rtt:?}");

    let reply = client.execute(b"status", 0.7).await.expect("execute failed");
    println!("{}", String::from_utf8_lossy(&reply));
}
```

### Modes

`Mode::Nosecure`, `Mode::Time` and `Mode::Challenge` correspond to the CLI
types 0, 1 and 2.

### API docs

```
cargo doc --no-deps --all-features --open
```

## Python module

Build and install it into a virtual environment with [maturin](https://www.maturin.rs/):

```
cd python
python -m venv .venv
. .venv/bin/activate
maturin develop --release
```

The module exposes `Client` (blocking), `AsyncClient` (asyncio), `Session`
(sans-IO), the helpers `colorize` and `parse_*`, and `ParseError`. Passwords
and commands are `bytes`; replies are `bytes`.

Modes: `0` = non-secure, `1` = time-secure (default), `2` = challenge-secure.

### Blocking client

```python
from xrcon import Client

client = Client("example.com", b"secret", mode=1, port=26000)

# Round trip time in seconds, or None on timeout.
print(client.ping())

# Run a command and print the reply.
print(client.execute(b"status"))
```

### Async client

```python
import asyncio
from xrcon import AsyncClient

async def main():
    # `async with` connects on entry and closes on exit.
    async with AsyncClient("example.com", b"secret", mode=1, port=26000) as client:
        print(await client.ping())
        print(await client.execute(b"status"))

asyncio.run(main())
```

### Session (bring your own socket)

`Session` does no network I/O and has no clock. You send the datagrams it
queues with `poll_transmit()`, feed received datagrams to `handle_datagram()`,
and pass the current time into each call. See `examples/python_session.py`.

### Helpers

`colorize(data)` converts Xonotic colour codes to ANSI escapes. The `parse_*`
functions turn raw packets or address text into Python values and raise
`ParseError` on bad input. See `examples/python_parse.py`.

### Docs

```
python -c "import xrcon; help(xrcon.Client)"
```

## Examples

All examples live in `examples/` and read their settings from environment
variables:

| Variable | Meaning |
|---|---|
| `XRCON_HOST` | Server host name or IP |
| `XRCON_PORT` | Server port |
| `XRCON_PASSWORD` | rcon password |
| `XRCON_MODE` | 0 = non-secure, 1 = time-secure, 2 = challenge-secure |

### Rust

```
XRCON_HOST=example.com XRCON_PORT=26000 XRCON_PASSWORD='secret' XRCON_MODE=1 \
  cargo run --example blocking
```

```
XRCON_HOST=example.com XRCON_PORT=26000 XRCON_PASSWORD='secret' XRCON_MODE=1 \
  cargo run --example nonblocking --features tokio
```

### Python

Install it via pip

```
pip install xrcon-rs
```

Build the module first (see "Python module"), then run:

```
XRCON_HOST=example.com XRCON_PORT=26000 XRCON_PASSWORD='secret' XRCON_MODE=1 \
  python examples/python_blocking.py
```

| File | Shows |
|---|---|
| `examples/blocking.rs` | Blocking Rust client |
| `examples/nonblocking.rs` | Async Rust client (needs the `tokio` feature) |
| `examples/python_blocking.py` | `Client` |
| `examples/python_async.py` | `AsyncClient` |
| `examples/python_session.py` | `Session` with your own UDP socket |
| `examples/python_parse.py` | `parse_*` helpers and `colorize` |

`python_parse.py` runs the `rcon` step only with `XRCON_MODE` 0 or 1.

## Nix

The flake provides the CLI package, a check and a development shell for
`x86_64-linux`, `aarch64-linux`, `x86_64-darwin` and `aarch64-darwin`.

### Build the CLI

```
nix build
./result/bin/xrcon -s example.com:26000 -p 'secret' status
```

### Run without installing

```
nix run . -- -s example.com:26000 -p 'secret' status
```

### Development shell

The shell provides `cargo`, `rustc`, `rustfmt`, `clippy`, `rust-analyzer`,
`maturin` and `python3`:

```
nix develop
```

Everything above (tests, examples, `maturin develop`, `cargo doc`) is run
inside this shell.

### Checks

```
nix flake check
```

Note: Nix only sees files that git knows about. Run `git add` on new files
before `nix build`.
