import { useState } from "react";
import { Mail, Settings, Inbox } from "lucide-react";
import { AccountSettings } from "@/features/accounts/AccountSettings";

type Page = "inbox" | "accounts";

function App() {
  const [page, setPage] = useState<Page>("accounts");

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-background text-foreground">
      {/* 侧边栏 */}
      <aside className="bg-sidebar flex w-56 shrink-0 flex-col border-r">
        <div className="flex h-14 items-center gap-2 border-b px-4">
          <Mail className="text-primary size-5" />
          <span className="font-semibold">LiveEmail</span>
        </div>

        <nav className="flex flex-col gap-1 p-2">
          <NavItem
            icon={<Inbox className="size-4" />}
            label="收件箱"
            active={page === "inbox"}
            onClick={() => setPage("inbox")}
            disabled
          />
          <NavItem
            icon={<Settings className="size-4" />}
            label="账户"
            active={page === "accounts"}
            onClick={() => setPage("accounts")}
          />
        </nav>

        <div className="text-muted-foreground mt-auto px-4 py-3 text-xs">
          第一批 · 账户体系
        </div>
      </aside>

      {/* 主内容 */}
      <main className="min-w-0 flex-1 overflow-y-auto">
        {page === "accounts" ? (
          <AccountSettings />
        ) : (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
            <Inbox className="text-muted-foreground size-8" />
            <p className="font-medium">收件箱将在第二批实现</p>
            <p className="text-muted-foreground text-sm">
              完成 IMAP 同步与邮件阅读后开放
            </p>
          </div>
        )}
      </main>
    </div>
  );
}

function NavItem({
  icon,
  label,
  active,
  onClick,
  disabled,
}: {
  icon: React.ReactNode;
  label: string;
  active: boolean;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={[
        "flex h-9 items-center gap-2 rounded-md px-3 text-sm font-medium transition-colors",
        active
          ? "bg-sidebar-accent text-sidebar-accent-foreground"
          : "text-sidebar-foreground hover:bg-sidebar-accent/60",
        disabled ? "opacity-50" : "",
      ].join(" ")}
    >
      {icon}
      {label}
    </button>
  );
}

export default App;
