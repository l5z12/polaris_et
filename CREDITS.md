# Credits

Polaris — https://github.com/l5z12/polaris_et

## App icon

The application icon (`assets/polaris.ico`, `assets/polaris256.png`, and the
MSIX logos in `packaging/Assets/`) is derived from **`ic_fluent_data_line_32`**
in Microsoft's **Fluent UI System Icons**, used under the MIT License.

- Project: https://github.com/microsoft/fluentui-system-icons
- © Microsoft Corporation

```
MIT License

Copyright (c) 2020 Microsoft Corporation

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Bundled Wintun driver

Polaris ships the official **Wintun** TUN driver (`wintun/bin/<arch>/wintun.dll`,
v0.14.1) next to the executable so EasyTier loads our WireGuard-signed copy
rather than a foreign `wintun.dll` found on the system `PATH`. The binary is
distributed unmodified under WireGuard LLC's Prebuilt Binaries License — see
`wintun/LICENSE.txt`.

- Project: https://www.wintun.net/
- © WireGuard LLC

## Other components

- **EasyTier** mesh-VPN core — https://github.com/EasyTier/EasyTier
- **windows-reactor** (WinUI 3 / windows-rs) — https://github.com/microsoft/windows-rs
