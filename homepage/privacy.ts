// Privacy policy content (rendered to /privacy.html + /en/ + /zh/ by build.ts).
// Short by design: Polaris collects nothing; the networking is EasyTier's, whose
// own policy is linked. The "last updated" date is NOT here — build.ts derives it
// from this file's last commit (git), so editing the policy below and committing
// is what moves the date. Keep the policy text in THIS file for that reason.

import type { Bi } from "./content.ts";

// EasyTier's own privacy policy — Chinese and English live at different paths.
export const EASYTIER_PRIVACY = {
  zh: "https://easytier.cn/guide/privacy.html",
  en: "https://easytier.cn/en/guide/privacy.html",
} satisfies Bi;

export const privacy = {
  title: { en: "Privacy Policy", zh: "隐私政策" } satisfies Bi,
  metaDescription: {
    en: "Privacy policy for Polaris — the app collects no data of any kind.",
    zh: "Polaris 隐私政策 —— 本应用不收集任何数据。",
  } satisfies Bi,

  body: [
    {
      en: "Polaris does not collect, store, or transmit any of your personal data. The app contains no analytics, telemetry, advertising, or tracking of any kind, and it sends no information to the developer or to any server the developer operates.",
      zh: "Polaris 不收集、存储或传输您的任何个人数据。本应用不含任何分析、遥测、广告或追踪，也不会向开发者或开发者运营的任何服务器发送任何信息。",
    },
    {
      en: "Everything stays on your device. Your network profiles, application settings, and diagnostic logs are written only to local storage on your own computer, and you can export or delete them at any time.",
      zh: "所有数据都保留在您的设备上。您的网络配置、应用设置和诊断日志仅写入您本机的本地存储，您可以随时导出或删除它们。",
    },
    {
      en: "Polaris is a desktop client for the EasyTier mesh VPN, with the EasyTier core embedded in the app. When you connect, your network traffic flows to the peers and relay servers that you configure — never to the Polaris developer. How that underlying networking handles data is governed by EasyTier, not by Polaris.",
      zh: "Polaris 是 EasyTier 网格 VPN 的桌面客户端，并将 EasyTier 内核内嵌于应用中。连接后，您的网络流量将流向您所配置的对等节点与中继服务器 —— 绝不会流向 Polaris 开发者。底层网络如何处理数据由 EasyTier 决定，而非 Polaris。",
    },
  ] satisfies Bi[],

  // {link} is replaced with the language-appropriate EasyTier privacy link.
  easytier: {
    en: "For how the embedded EasyTier engine handles data, see the {link}.",
    zh: "有关内嵌的 EasyTier 内核如何处理数据，请参阅 {link}。",
  } satisfies Bi,
  easytierLink: { en: "EasyTier Privacy Policy", zh: "EasyTier 隐私政策" } satisfies Bi,

  contact: {
    en: "Questions about privacy? Open an issue on the project's GitHub repository.",
    zh: "对隐私有疑问？请在项目的 GitHub 仓库提交 issue。",
  } satisfies Bi,

  back: { en: "← Back to home", zh: "← 返回首页" } satisfies Bi,
};
