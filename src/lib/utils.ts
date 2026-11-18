import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { format, isThisYear, isToday, isYesterday } from "date-fns";
import { zhCN } from "date-fns/locale";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** 邮件列表时间：今天显示时分，昨天显示“昨天”，今年显示月日，更早显示年月日 */
export function formatMailDate(timestamp: number | null): string {
  if (!timestamp) return "";
  const date = new Date(timestamp * 1000);
  if (isToday(date)) return format(date, "HH:mm");
  if (isYesterday(date)) return "昨天";
  if (isThisYear(date)) return format(date, "M月d日");
  return format(date, "yyyy/M/d");
}

export function formatFullDate(timestamp: number | null): string {
  if (!timestamp) return "";
  return format(new Date(timestamp * 1000), "yyyy-MM-dd HH:mm", { locale: zhCN });
}

export function formatBytes(bytes: number): string {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / Math.pow(1024, index);
  return `${value >= 10 || index === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[index]}`;
}

export function senderLabel(name: string, addr: string): string {
  if (name && name.trim()) return name.trim();
  if (addr) return addr.split("@")[0];
  return "(未知发件人)";
}

export function initials(text: string): string {
  const value = text.trim();
  if (!value) return "?";
  return value.slice(0, 1).toUpperCase();
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** 生成引用正文（HTML） */
export function quoteHtml(html: string | null, text: string | null, meta: string): string {
  const content = html
    ? html
    : `<pre style="white-space:pre-wrap;font-family:inherit">${escapeHtml(text ?? "")}</pre>`;
  return `<p><br></p><p>${escapeHtml(meta)}</p><blockquote>${content}</blockquote>`;
}

export function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/** 把纯文本转成简单 HTML 段落 */
export function textToHtml(text: string): string {
  return text
    .split(/\n{2,}/)
    .map((paragraph) => `<p>${escapeHtml(paragraph).replace(/\n/g, "<br>")}</p>`)
    .join("");
}

export function htmlToText(html: string): string {
  const doc = new DOMParser().parseFromString(html, "text/html");
  return doc.body.textContent ?? "";
}
