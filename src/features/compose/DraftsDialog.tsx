import { useQuery, useQueryClient } from "@tanstack/react-query";
import { FileText, Loader2, Trash2 } from "lucide-react";
import { composeApi } from "@/lib/api";
import { formatFullDate } from "@/lib/utils";
import { useAppStore, toast } from "@/store/app";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export function DraftsDialog() {
  const { setShowDrafts, openCompose } = useAppStore();
  const queryClient = useQueryClient();
  const { data, isLoading } = useQuery({
    queryKey: ["drafts"],
    queryFn: composeApi.drafts,
  });

  const remove = async (id: number) => {
    if (!window.confirm("确定删除这份草稿吗？")) return;
    try {
      await composeApi.deleteDraft(id);
      void queryClient.invalidateQueries({ queryKey: ["drafts"] });
    } catch (e) {
      toast.error(String(e));
    }
  };

  return (
    <Dialog open onOpenChange={(value) => !value && setShowDrafts(false)}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle className="text-base">草稿箱</DialogTitle>
        </DialogHeader>
        {isLoading ? (
          <div className="text-muted-foreground flex items-center gap-2 py-8 text-sm">
            <Loader2 className="size-4 animate-spin" />
            正在加载…
          </div>
        ) : !data || data.length === 0 ? (
          <div className="flex flex-col items-center gap-2 py-12 text-center">
            <FileText className="text-muted-foreground size-8" />
            <p className="text-sm">没有草稿</p>
          </div>
        ) : (
          <ul className="max-h-[60vh] divide-y overflow-y-auto">
            {data.map((draft) => (
              <li key={draft.id} className="flex items-center gap-3 py-2.5">
                <button
                  className="min-w-0 flex-1 text-left"
                  onClick={() => {
                    setShowDrafts(false);
                    openCompose({
                      mode: "draft",
                      accountId: draft.account_id,
                      draftId: draft.id,
                    });
                  }}
                >
                  <div className="truncate text-sm font-medium">
                    {draft.subject || "(无主题)"}
                  </div>
                  <div className="text-muted-foreground truncate text-xs">
                    收件人：{draft.to_addrs || "—"} · {formatFullDate(draft.updated_at)}
                  </div>
                </button>
                <button
                  onClick={() => void remove(draft.id)}
                  className="text-destructive hover:bg-destructive/10 inline-flex size-8 items-center justify-center rounded"
                  aria-label="删除草稿"
                >
                  <Trash2 className="size-4" />
                </button>
              </li>
            ))}
          </ul>
        )}
      </DialogContent>
    </Dialog>
  );
}
