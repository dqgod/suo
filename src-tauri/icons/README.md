# Icon assets

`icon.png`, `icon.ico`, and the `Square*Logo.png` files are the shared and
Windows icon assets. Do not replace them when adjusting only the macOS Dock
appearance.

`icon-macos.png` is the macOS source master. Its existing artwork is scaled to
the 824 x 824 macOS visual keyline and centered on a transparent 1024 x 1024
canvas. `icon.icns` is generated from that padded master.

To regenerate the macOS bundle icon without changing the verified Windows
assets, generate into a temporary directory and copy back only `icon.icns`:

```bash
output_dir="$(mktemp -d /tmp/suo-tauri-icons.XXXXXX)"
pnpm tauri icon src-tauri/icons/icon-macos.png --output "$output_dir"
cp "$output_dir/icon.icns" src-tauri/icons/icon.icns
```
