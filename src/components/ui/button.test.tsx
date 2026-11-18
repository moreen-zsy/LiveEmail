import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

describe("Button", () => {
  it("applies variant styles and fires click handlers", () => {
    const onClick = vi.fn();
    render(
      <Button variant="destructive" onClick={onClick}>
        删除
      </Button>,
    );

    const button = screen.getByRole("button", { name: "删除" });
    expect(button).toHaveClass("bg-destructive");
    fireEvent.click(button);
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("renders a child element when asChild is set", () => {
    render(
      <Button asChild>
        <a href="/settings">设置</a>
      </Button>,
    );

    expect(screen.getByRole("link", { name: "设置" })).toHaveAttribute(
      "href",
      "/settings",
    );
  });

  it("respects disabled state", () => {
    const onClick = vi.fn();
    render(
      <Button disabled onClick={onClick}>
        保存
      </Button>,
    );

    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(onClick).not.toHaveBeenCalled();
  });
});

describe("Badge", () => {
  it("renders text and variant class", () => {
    render(<Badge variant="secondary">未读</Badge>);
    const badge = screen.getByText("未读");
    expect(badge).toBeInTheDocument();
    expect(badge).toHaveClass("bg-secondary");
  });
});
