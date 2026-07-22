# vesper

**vesper — Professional OSINT Username Scanner**

A high-performance username investigation tool that searches across 2000+ social networks and websites. Built in Rust with async concurrency, a polished CLI, and professional-grade logging.

[![forthebadge](https://forthebadge.com/images/badges/made-with-rust.svg)](https://forthebadge.com)

<p align="center">
  <img src="https://raw.githubusercontent.com/krishpranav/vesper/master/images/vesper.png" height="200"/>
</p>

## ✨ Features

- 🔎 **Comprehensive Coverage**: Search across 2000+ social networks and platforms
- ⚡ **Blazing Fast**: Async concurrent scanning with configurable worker pools (32 workers default)
- 🎨 **Beautiful CLI**: Professional OSINT-style output with colors, progress tracking, and structured logging
- 🔒 **Privacy-Focused**: Optional Tor proxy support for anonymous scanning
- 📸 **Screenshot Capture**: Automated headless Chrome screenshots of found profiles
- 📥 **Content Download**: Download profile data from supported sites (Instagram, etc.)
- 🧾 **JSON Reports**: Save scan results to a machine-readable report file
- 🧪 **Site Validation**: Built-in test mode to verify site configurations

## 🚀 Installation

### Prerequisites

- **Rust 1.70+** - [Install Rust](https://rustup.rs/)
- **Chrome/Chromium** (optional, for screenshots) - Version 60+

### Build from Source

```bash
git clone https://github.com/krishpranav/vesper
cd vesper
cargo build --release
```

The compiled binary will be available at `./target/release/vesper`

### Install Globally

```bash
cargo install --path .
```

## 📖 Usage

### Basic Scan

```bash
vesper krishpranav
```

### Scan Multiple Usernames

```bash
vesper krishpranav blue red
```

### Verbose Output (Show Not Found Sites)

```bash
vesper user -v
```

### Specific Site Only

```bash
vesper user --site github
```

### With Tor Proxy

Requires Tor running on `127.0.0.1:9050`

```bash
vesper user --tor
```

### Capture Screenshots

```bash
vesper user --screenshot
```

Screenshots will be saved to `screenshots/<username>/`

### Download Content

```bash
vesper user --download
```

### Update Database

```bash
vesper user --update
```

### Save Report

```bash
# Auto-detected from file extension
vesper user --output report.json     # JSON format
vesper user --output results.csv     # CSV format (for spreadsheets/OSINT tools)
vesper user --output results.txt     # Human-readable text report

# Explicit format override
vesper user --output results --format csv
```

### Test Mode (Validate Site Configurations)

```bash
vesper --test
```

### All Options

```bash
vesper --help
```

## 🎯 Example Output

```
    ██╗   ██╗ ███████╗ ██████╗ ██████╗ ███████╗ ██████╗ 
    ██║   ██║ ██╔════╝██╔════╝ ██╔══██╗ ██╔════╝ ██╔══██╗
    ██║   ██║ █████╗  ╚█████╗  ██████╔╝ █████╗  ██████╔╝
    ╚██╗ ██╔╝ ██╔══╝   ╚═══██╗ ██╔═══╝  ██╔══╝  ██╔══██╗
     ╚████╔╝  ███████╗ ██████╔╝ ██║      ███████╗ ██║  ██║
      ╚═══╝   ╚══════╝ ╚═════╝  ╚═╝      ╚══════╝ ╚═╝  ╚═╝
    
    🔎 vesper — Professional OSINT Username Scanner

🔎 Investigating user on:

[+] GitHub: https://www.github.com/user
[+] Instagram: https://www.instagram.com/user
[+] Twitter: https://twitter.com/user
[-] Pinterest: Not Found!

═══════════════════════════════════════
  🧠 SCAN COMPLETE
═══════════════════════════════════════
  Found: 12
  Checked: 2300
  Time: 3.2s
═══════════════════════════════════════
```

## 🛠️ CLI Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--help` | `-h` | Show help message |
| `--version` | `-V` | Show version |
| `--no-color` | | Disable colored output |
| `--verbose` | `-v` | Show not found sites |
| `--tor` | `-t` | Use Tor proxy (127.0.0.1:9050) |
| `--screenshot` | `-s` | Take screenshots of found profiles |
| `--download` | `-d` | Download profile content |
| `--output <FILE>` | `-o` | Save scan results (format auto-detected from extension) |
| `--format <FORMAT>` | | Export format: `json`, `csv`, or `txt` (overrides extension) |
| `--update` | | Update site database from Sherlock |
| `--database <PATH>` | | Use custom database file |
| `--site <SITE>` | | Check specific site only |
| `--test` | | Run site validation tests |

## 🔧 Configuration

### Worker Pool Size

- **Default**: 32 concurrent workers
- **With Screenshots**: 8 workers (automatically reduced)

### Tor Proxy

Default proxy address: `socks5://127.0.0.1:9050`

To use Tor:
1. Install and start Tor service
2. Run vesper with `--tor` flag

## 📊 Performance

The Rust implementation provides:
- **Async I/O**: Non-blocking concurrent requests using Tokio
- **Memory Efficient**: Minimal allocations with zero-copy where possible
- **Fast Startup**: Compiled binary with instant execution
- **Resource Control**: Configurable concurrency limits

## 🎨 Technology Stack

- **Runtime**: [Tokio](https://tokio.rs/) - Async runtime
- **HTTP**: [Reqwest](https://docs.rs/reqwest/) - HTTP client with SOCKS5 support
- **CLI**: [Clap](https://docs.rs/clap/) - Command-line argument parsing
- **Logging**: [Tracing](https://docs.rs/tracing/) - Structured logging
- **UI**: [Indicatif](https://docs.rs/indicatif/) + [Console](https://docs.rs/console/) + [Colored](https://docs.rs/colored/) - Beautiful terminal output
- **Screenshots**: [Headless Chrome](https://docs.rs/headless_chrome/) - Browser automation
- **Regex**: [Fancy Regex](https://docs.rs/fancy-regex/) - Advanced pattern matching

## 📝 Data Files

The following files are **unchanged** from upstream:

- `data.json` - Site database (2000+ sites)
- `sites.md` - Site documentation
- `generate_sites_md.py` - Site list generator

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📜 License

MIT License - see LICENSE file for details

## 🔗 Related Projects

- [Sherlock](https://github.com/sherlock-project/sherlock) - Python OSINT tool
- [WhatsMyName](https://github.com/WebBreacher/WhatsMyName) - Username enumeration

---

**Made with ❤️ and Rust**
