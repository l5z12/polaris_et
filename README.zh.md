<p align="center">
  <img src="assets/data-line.svg" width="76" alt="Polaris" />
</p>

<h1 align="center">Polaris</h1>

<p align="center"><a href="README.md">English</a> · <b>简体中文</b></p>

一个快速、友好的 **WinUI 3** 桌面客户端，用于 [EasyTier](https://github.com/EasyTier/EasyTier)
网格 VPN —— EasyTier 内核**直接内嵌在应用中**，因此无需单独安装或管理
`easytier-core.exe` 守护进程。

> Polaris 是自由软件，依据 **GNU GPL v3.0** 许可。

## 功能特性

- **内嵌 EasyTier 内核** —— 网格 VPN 引擎作为 Rust 库在进程内运行；无需外部
  守护进程或命令行工具。
- **同时连接多个网络** —— 可同时连接多个 EasyTier 网络，每个网络拥有独立配置。
- **VPN 或代理** —— 以管理员身份运行时通过 TUN 适配器提供完整 VPN；否则
  SOCKS5 和端口转发代理仍可使用，无需管理员权限。
- **P2P 网格** —— TCP / UDP / WebSocket 传输、WireGuard 加密、子网代理、
  SOCKS5 门户以及 Magic DNS。
- **原生 Windows 体验** —— 系统托盘图标、单实例处理、配置导入/导出、
  浅色/深色主题以及 Mica/Acrylic 材质。
- **内置 Wintun 驱动** —— 随附经 WireGuard 签名的 `wintun.dll`，使 TUN 适配器
  确定性地加载，而非加载 `PATH` 中找到的第三方副本。

## 系统要求

- Windows 10 2004 版（内部版本 19041）或更高版本。提权（具备 VPN 能力）的 MSIX
  版本需要 **Windows 11**。
- **Windows App SDK** 运行时（WinUI 3）—— 安装该运行时，或将 WinAppSDK 的 DLL
  放在可执行文件旁边。

## 构建

Polaris 使用 Cargo 和较新的 Rust 工具链构建（edition 2024，Rust 1.85+）：

```powershell
cargo build --release --bin polaris_et
cargo run --release
```

构建会从 Git 拉取 `windows-reactor` 和 `easytier`（见 `Cargo.toml`），并通过
`build.rs` 嵌入应用图标，将随附的 `wintun.dll` 放到生成的可执行文件旁边。

### 管理员 / VPN

创建 TUN 适配器需要管理员权限。Polaris 从不强制提权：

- **没有管理员权限**时，它以仅代理模式（SOCKS5 / 端口转发）运行，连接时不会报错。
- 启用 **设置 → 始终以管理员身份启动**，或点击 **立即以管理员身份重启**，以启用
  完整 VPN 模式。

## 打包（MSIX）

Polaris 可以打包为 MSIX，用于 Microsoft Store 或旁加载，有两种形态：

| 构建 | TUN VPN | 代理 |
| --- | --- | --- |
| `cargo build --release`（仅代理） | 否 | 是 |
| `cargo build --release --features msix`（具备 VPN 能力，Win 11+） | 是 | 是 |

具备 VPN 能力的构建嵌入了 `highestAvailable` UAC 清单，并搭配 `allowElevation`
能力，使打包后的应用可以提权。完整的布局 / 打包 / 签名步骤以及 Windows 11 和
商店认证的注意事项，请参见 **[packaging/PACKAGING.md](packaging/PACKAGING.md)**。

## 项目结构

| 路径 | 说明 |
| --- | --- |
| `src/main.rs` | 应用外壳、根状态、渲染循环、启动与提权 |
| `src/ui.rs` | WinUI 页面（网络、设置、关于） |
| `src/engine.rs` | EasyTier 实例生命周期 |
| `src/config.rs` | 网络配置与持久化设置 |
| `src/elevate.rs` | 权限与 MSIX 打包检测 |
| `src/instance.rs` | 单实例协调 |
| `src/tray.rs` / `src/dialog.rs` | 托盘图标、原生文件对话框 |
| `build.rs` | 嵌入图标、随附 `wintun.dll`、MSIX UAC 清单 |

## 许可证

Polaris 依据 **GNU 通用公共许可证第 3 版** 授权 —— 参见 [`LICENSE`](LICENSE)。
发布本软件是希望它能有用，但不提供任何担保。

第三方组件及其许可证（EasyTier、windows-reactor、Wintun 驱动以及 Fluent UI
图标）列于 [`CREDITS.md`](CREDITS.md)。
