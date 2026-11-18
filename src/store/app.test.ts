import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore, useToastStore } from "@/store/app";

const initialState = {
  view: "mail" as const,
  accountId: null,
  folderId: null,
  selectedId: null,
  search: "",
  unreadOnly: false,
  flaggedOnly: false,
  hasAttachment: false,
  selection: new Set<number>(),
  compose: null,
  showDrafts: false,
};

beforeEach(() => {
  useAppStore.setState(initialState);
  useToastStore.setState({ toasts: [] });
});

describe("useAppStore", () => {
  it("selectAccount resets folder, selection and view", () => {
    useAppStore.setState({
      view: "settings",
      folderId: 9,
      selectedId: 3,
      selection: new Set([1, 2]),
    });

    useAppStore.getState().selectAccount(7);

    const state = useAppStore.getState();
    expect(state.accountId).toBe(7);
    expect(state.folderId).toBeNull();
    expect(state.selectedId).toBeNull();
    expect(state.selection.size).toBe(0);
    expect(state.view).toBe("mail");
  });

  it("selectFolder sets account and folder", () => {
    useAppStore.getState().selectFolder(2, 5);
    const state = useAppStore.getState();
    expect(state.accountId).toBe(2);
    expect(state.folderId).toBe(5);
    expect(state.view).toBe("mail");
  });

  it("toggles message selection without mutating previous set", () => {
    const store = useAppStore.getState();
    store.toggleSelection(1);
    const first = useAppStore.getState().selection;
    store.toggleSelection(2);
    const second = useAppStore.getState().selection;

    expect([...second].sort()).toEqual([1, 2]);
    expect([...first]).toEqual([1]);
    expect(second).not.toBe(first);

    useAppStore.getState().toggleSelection(1);
    expect([...useAppStore.getState().selection]).toEqual([2]);
  });

  it("sets and clears selection", () => {
    useAppStore.getState().setSelection([3, 4]);
    expect([...useAppStore.getState().selection]).toEqual([3, 4]);
    useAppStore.getState().clearSelection();
    expect(useAppStore.getState().selection.size).toBe(0);
  });

  it("opens and closes compose with intent", () => {
    useAppStore.getState().openCompose({ mode: "reply", accountId: 1, draftId: 9 });
    expect(useAppStore.getState().compose).toEqual({
      mode: "reply",
      accountId: 1,
      draftId: 9,
    });
    useAppStore.getState().closeCompose();
    expect(useAppStore.getState().compose).toBeNull();
  });

  it("updates search and filters independently", () => {
    const store = useAppStore.getState();
    store.setSearch("发票");
    store.setUnreadOnly(true);
    store.setFlaggedOnly(true);
    store.setHasAttachment(true);

    const state = useAppStore.getState();
    expect(state.search).toBe("发票");
    expect(state.unreadOnly).toBe(true);
    expect(state.flaggedOnly).toBe(true);
    expect(state.hasAttachment).toBe(true);
  });
});

describe("useToastStore", () => {
  it("pushes, auto-dismisses after 5s and can be dismissed manually", () => {
    vi.useFakeTimers();
    try {
      const toast = useToastStore.getState();
      toast.push("保存成功", "success");
      toast.push("出错了", "error");

      expect(useToastStore.getState().toasts).toHaveLength(2);
      expect(useToastStore.getState().toasts[0]).toMatchObject({
        kind: "success",
        text: "保存成功",
      });

      vi.advanceTimersByTime(5000);
      expect(useToastStore.getState().toasts).toHaveLength(0);

      toast.push("手动关闭");
      const id = useToastStore.getState().toasts[0].id;
      useToastStore.getState().dismiss(id);
      expect(useToastStore.getState().toasts).toHaveLength(0);
    } finally {
      vi.useRealTimers();
    }
  });
});
