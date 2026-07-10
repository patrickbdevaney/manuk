# Desktop integration

To make Manuk launchable from the GNOME/KDE app grid (and as a browser you can open
links with), install the binary and the desktop entry:

```bash
# Build an optimized binary
cargo build --release -p manuk-shell            # -> target/release/manuk

# Put it on PATH (or adjust Exec= in the .desktop to an absolute path)
install -Dm755 target/release/manuk ~/.local/bin/manuk

# Install the launcher entry
install -Dm644 packaging/manuk.desktop ~/.local/share/applications/manuk.desktop
update-desktop-database ~/.local/share/applications 2>/dev/null || true
```

Launching **Manuk** from the app grid (no URL) opens the home / new-tab page with the
address bar focused — type a URL or a search and press Enter. Opening a link routes
`manuk browse <url>`.

To make it the default browser for `http(s)` links:

```bash
xdg-settings set default-web-browser manuk.desktop
```
