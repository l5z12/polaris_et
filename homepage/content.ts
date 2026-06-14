// Site content, in English + Simplified Chinese. The build (build.ts) renders
// BOTH languages into the static HTML; the active one is chosen at runtime
// (auto-detected, then remembered) — see src/main.ts and public/styles.css.

export type Bi = { en: string; zh: string };

export const REPO = "https://github.com/l5z12/polaris_et";
export const RELEASES = `${REPO}/releases`;

// Privacy policy content + its EasyTier link live in ./privacy.ts (so its git
// history drives the policy's "last updated" date independently of site copy).

export const site = {
  name: "Polaris",
  // <title> / meta description use the English copy (the canonical language).
  description:
    "A fast, friendly WinUI 3 desktop client for the EasyTier mesh VPN, with the EasyTier core embedded directly in the app.",

  tagline: {
    en: "A fast, friendly WinUI 3 client for the EasyTier mesh VPN — with the core embedded right in the app.",
    zh: "一个快速、友好的 EasyTier 网格 VPN 的 WinUI 3 客户端 —— 内核直接内嵌在应用中。",
  } satisfies Bi,

  download: { en: "Download", zh: "下载" } satisfies Bi,
  viewSource: { en: "View on GitHub", zh: "在 GitHub 查看" } satisfies Bi,
  // Label on the language switch shows the language you'd switch *to*.
  switchTo: { en: "中文", zh: "English" } satisfies Bi,

  featuresHeading: { en: "Features", zh: "功能特性" } satisfies Bi,

  features: [
    {
      title: { en: "Embedded EasyTier core", zh: "内嵌 EasyTier 内核" },
      body: {
        en: "The mesh-VPN engine runs in-process as a Rust library — no separate easytier-core.exe daemon to install or manage.",
        zh: "网格 VPN 引擎作为 Rust 库在进程内运行 —— 无需单独安装或管理 easytier-core.exe 守护进程。",
      },
    },
    {
      title: { en: "Multiple networks at once", zh: "同时连接多个网络" },
      body: {
        en: "Connect to several EasyTier networks simultaneously, each with its own profile.",
        zh: "可同时连接多个 EasyTier 网络，每个网络拥有独立配置。",
      },
    },
    {
      title: { en: "VPN or proxy", zh: "VPN 或代理" },
      body: {
        en: "Full VPN via a TUN adapter when run as administrator; otherwise SOCKS5 and port-forward proxies still work, no admin required.",
        zh: "以管理员身份运行时通过 TUN 适配器提供完整 VPN；否则 SOCKS5 和端口转发代理仍可使用，无需管理员权限。",
      },
    },
    {
      title: { en: "P2P mesh", zh: "P2P 网格" },
      body: {
        en: "TCP / UDP / WebSocket transports, WireGuard crypto, subnet proxy, a SOCKS5 portal, and magic DNS.",
        zh: "TCP / UDP / WebSocket 传输、WireGuard 加密、子网代理、SOCKS5 门户以及 Magic DNS。",
      },
    },
    {
      title: { en: "Native Windows UX", zh: "原生 Windows 体验" },
      body: {
        en: "System-tray icon, single-instance handling, config import/export, light/dark themes and Mica/Acrylic materials.",
        zh: "系统托盘图标、单实例处理、配置导入/导出、浅色/深色主题以及 Mica/Acrylic 材质。",
      },
    },
    {
      title: { en: "Bundled Wintun driver", zh: "内置 Wintun 驱动" },
      body: {
        en: "Ships the WireGuard-signed wintun.dll so the TUN adapter loads deterministically instead of a foreign copy on PATH.",
        zh: "随附经 WireGuard 签名的 wintun.dll，使 TUN 适配器确定性地加载，而非加载 PATH 中的第三方副本。",
      },
    },
  ] satisfies { title: Bi; body: Bi }[],

  requirements: {
    en: "Windows 10 2004 (build 19041)+ and the Windows App SDK runtime. Windows 11 is required for the VPN-capable MSIX build.",
    zh: "Windows 10 2004 版（内部版本 19041）或更高版本，以及 Windows App SDK 运行时。具备 VPN 能力的 MSIX 版本需要 Windows 11。",
  } satisfies Bi,

  license: {
    en: "Free software under the GNU GPL v3.0.",
    zh: "依据 GNU GPL v3.0 许可的自由软件。",
  } satisfies Bi,

  privacyLink: { en: "Privacy Policy", zh: "隐私政策" } satisfies Bi,
};
