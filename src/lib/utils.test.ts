import { describe, expect, it } from "vitest";
import {
  cn,
  escapeHtml,
  formatBytes,
  formatMailDate,
  htmlToText,
  initials,
  quoteHtml,
  senderLabel,
  textToHtml,
} from "@/lib/utils";

describe("formatMailDate", () => {
  it("returns empty string for null", () => {
    expect(formatMailDate(null)).toBe("");
  });

  it("formats today as HH:mm", () => {
    const now = Math.floor(Date.now() / 1000);
    expect(formatMailDate(now)).toMatch(/^\d{2}:\d{2}$/);
  });

  it("formats yesterday", () => {
    const yesterday = Math.floor(Date.now() / 1000) - 24 * 60 * 60;
    expect(formatMailDate(yesterday)).toBe("昨天");
  });

  it("formats older dates with year", () => {
    const old = Math.floor(new Date(2020, 0, 2, 12).getTime() / 1000);
    expect(formatMailDate(old)).toBe("2020/1/2");
  });
});

describe("formatBytes", () => {
  it("handles zero and small values", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
  });

  it("uses binary units", () => {
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(1024 * 1024 * 2)).toBe("2.0 MB");
    expect(formatBytes(10 * 1024 * 1024)).toBe("10 MB");
  });
});

describe("sender helpers", () => {
  it("prefers display name, then address local part", () => {
    expect(senderLabel(" 张三 ", "zhang@example.com")).toBe("张三");
    expect(senderLabel("", "zhang@example.com")).toBe("zhang");
    expect(senderLabel("", "")).toBe("(未知发件人)");
  });

  it("builds initials", () => {
    expect(initials("alice")).toBe("A");
    expect(initials(" 张三 ")).toBe("张");
    expect(initials("   ")).toBe("?");
  });
});

describe("html helpers", () => {
  it("escapes html entities", () => {
    expect(escapeHtml(`<a href="x">'&'</a>`)).toBe(
      "&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;",
    );
  });

  it("converts plain text paragraphs to html", () => {
    expect(textToHtml("hello\nworld\n\nsecond")).toBe(
      "<p>hello<br>world</p><p>second</p>",
    );
  });

  it("extracts text from html", () => {
    expect(htmlToText("<p>Hello <b>world</b></p>")).toBe("Hello world");
  });

  it("quotes original content and escapes metadata", () => {
    const quoted = quoteHtml("<p>原始内容</p>", null, "张三 <a@b.com> 写道：");
    expect(quoted).toContain("<blockquote><p>原始内容</p></blockquote>");
    expect(quoted).toContain("&lt;a@b.com&gt;");
    expect(quoted).not.toContain("<a@b.com>");
  });

  it("falls back to text when html is missing", () => {
    const quoted = quoteHtml(null, "a < b", "meta");
    expect(quoted).toContain('<pre style="white-space:pre-wrap');
    expect(quoted).toContain("a &lt; b");
  });
});

describe("cn", () => {
  it("merges conflicting tailwind classes", () => {
    expect(cn("px-2", "px-4")).toBe("px-4");
    expect(cn("text-sm", false && "hidden", "font-bold")).toBe(
      "text-sm font-bold",
    );
  });
});
