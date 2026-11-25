import { describe, expect, it } from "vitest";
import { LiveEditBuffer } from "./live-edit";

function end(length: number) {
  return { start: length, end: length };
}

describe("live keyboard buffer", () => {
  it("sends only the new suffix, counting supplementary characters in UTF-8 bytes", () => {
    const editor = new LiveEditBuffer({ textBytes: 4, backspace: true });
    expect(editor.input("a", "insertText", false)).toEqual({ kind: "insert", text: "a" });
    expect(editor.input("a🐾", "insertText", false)).toEqual({ kind: "insert", text: "🐾" });
    expect(editor.input("a🐾\n", "insertLineBreak", false)).toEqual({ kind: "insert", text: "\n" });
    expect(editor.input("a🐾\n🐾a", "insertText", false)).toEqual({ kind: "compose" });
    expect(editor.input("a🐾\n🐾ab", "insertText", false)).toBeUndefined();
    editor.reset();
    expect(editor.input("b", "insertText", false)).toEqual({ kind: "insert", text: "b" });
  });

  it("validates the negotiated limit and rejects malformed or excessive local text", () => {
    for (const textBytes of [-1, 4097, 1.5, Number.NaN]) {
      expect(() => new LiveEditBuffer({ textBytes, backspace: true })).toThrow(RangeError);
    }
    const editor = new LiveEditBuffer({ textBytes: 4096, backspace: true });
    for (const value of ["\ud800", "\udfff", "a".repeat(4097)]) {
      expect(editor.input(value, "insertText", false)).toEqual({ kind: "compose" });
      editor.reset();
    }
  });

  it("maps a verified trailing ASCII removal and one cancelable empty backspace", () => {
    const editor = new LiveEditBuffer({ textBytes: 4096, backspace: true });
    editor.input("ab", "insertText", false);
    expect(editor.emptyBackspace("deleteContentBackward", end(2), true)).toBeUndefined();
    editor.beforeInput(end(2));
    expect(editor.input("a", "deleteContentBackward", false)).toEqual({
      kind: "delete-backward",
      repeatCount: 1,
    });
    expect(editor.input("a", "deleteContentBackward", false)).toBeUndefined();
    expect(editor.input("", "deleteContentBackward", false)).toEqual({
      kind: "delete-backward",
      repeatCount: 1,
    });
    expect(editor.emptyBackspace("deleteContentBackward", end(0), true)).toEqual({
      kind: "delete-backward",
      repeatCount: 1,
    });
    expect(editor.emptyBackspace("deleteContentBackward", end(0), false)).toBeUndefined();
    expect(editor.emptyBackspace("deleteContentForward", end(0), true)).toBeUndefined();
    const unsupported = new LiveEditBuffer({ textBytes: 4096, backspace: false });
    expect(unsupported.emptyBackspace("deleteContentBackward", end(0), true)).toBeUndefined();
    unsupported.input("a", "insertText", false);
    expect(unsupported.input("", "deleteContentBackward", false)).toEqual({ kind: "compose" });
  });

  it("keeps live typing active through Safari-style repeated deletion", () => {
    const editor = new LiveEditBuffer({ textBytes: 4096, backspace: true });
    editor.input("one two three", "insertText", false);
    editor.beforeInput(end(13));
    expect(editor.input("one two", "deleteWordBackward", false)).toEqual({
      kind: "delete-backward",
      repeatCount: 6,
    });
    editor.beforeInput(end(7));
    expect(editor.input("one", "deleteContentBackward", false)).toEqual({
      kind: "delete-backward",
      repeatCount: 4,
    });
    editor.beforeInput(end(3));
    expect(editor.input("on", "deleteContentBackward", false)).toEqual({
      kind: "delete-backward",
      repeatCount: 1,
    });
    expect(editor.input("on!", "insertText", false)).toEqual({ kind: "insert", text: "!" });
  });

  it.each([
    { before: "aaaa", after: "aa", selection: { start: 0, end: 2 } },
    { before: "aaaa", after: "aa", selection: undefined },
    { before: "🐾a", after: "", selection: end(3) },
    { before: "a".repeat(65), after: "", selection: end(65) },
  ])("keeps ambiguous or oversized repeated deletion local", ({ before, after, selection }) => {
    const editor = new LiveEditBuffer({ textBytes: 4096, backspace: true });
    editor.input(before, "insertText", false);
    if (selection) editor.beforeInput(selection);
    expect(editor.input(after, "deleteContentBackward", false)).toEqual({ kind: "compose" });
  });

  it("accepts selected trailing words and input-only accelerated deletes, then ordinary taps", () => {
    const editor = new LiveEditBuffer({ textBytes: 4096, backspace: true });
    editor.input("one two three", "insertText", false);
    editor.beforeInput({ start: 7, end: 13 });
    expect(editor.input("one two", "deleteWordBackward", false, end(7))).toEqual({
      kind: "delete-backward",
      repeatCount: 6,
    });
    expect(editor.input("one", "deleteSoftLineBackward", false, end(3))).toEqual({
      kind: "delete-backward",
      repeatCount: 4,
    });
    expect(editor.input("", "deleteContentBackward", false, end(0))).toEqual({
      kind: "delete-backward",
      repeatCount: 3,
    });
    expect(editor.emptyBackspace("deleteWordBackward", end(0), true)).toEqual({
      kind: "delete-backward",
      repeatCount: 1,
    });
    expect(editor.input("ok", "insertText", false)).toEqual({ kind: "insert", text: "ok" });
  });

  it("keeps input-only mid-string deletes local even with matching repeated characters", () => {
    const editor = new LiveEditBuffer({ textBytes: 4096, backspace: true });
    editor.input("aaaa", "insertText", false);
    expect(editor.input("aa", "deleteContentBackward", false, end(1))).toEqual({ kind: "compose" });
  });

  it.each(["🐾", "e\u0301", "👩\u200d💻"])("keeps deletion of %s local", (value) => {
    const editor = new LiveEditBuffer({ textBytes: 4096, backspace: true });
    editor.input(value, "insertText", false);
    editor.beforeInput(end(value.length));
    expect(editor.input("", "deleteContentBackward", false)).toEqual({ kind: "compose" });
  });

  it.each(["insertReplacementText", "insertFromPaste", "historyUndo"])(
    "keeps ambiguous %s operations local even when their result is an append",
    (type) => {
      const editor = new LiveEditBuffer({ textBytes: 4096, backspace: true });
      editor.input("a", "insertText", false);
      expect(editor.input("ab", type, false)).toEqual({ kind: "compose" });
    },
  );

  it("uses observed beforeinput selection when deciding whether an edit is a suffix", () => {
    const editor = new LiveEditBuffer({ textBytes: 4096, backspace: true });
    editor.input("a", "insertText", false);
    editor.beforeInput({ start: 0, end: 1 });
    expect(editor.input("ab", "insertText", false)).toEqual({ kind: "compose" });
  });

  it("sends confirmed composition exactly once whether its DOM commit arrives early or late", () => {
    const editor = new LiveEditBuffer({ textBytes: 4096, backspace: true });
    editor.input("a", "insertText", false);
    editor.compositionStart(end(1));
    expect(editor.input("an", "insertCompositionText", true)).toBeUndefined();
    expect(editor.input("a猫", "insertCompositionText", false)).toBeUndefined();
    expect(editor.compositionEnd("a猫", "猫")).toEqual({ kind: "insert", text: "猫" });
    expect(editor.input("a猫", "insertText", false)).toBeUndefined();

    editor.compositionStart(end(2));
    expect(editor.compositionEnd("a猫b", "犬")).toBeUndefined();
    expect(editor.input("a猫犬", "insertCompositionText", false)).toEqual({
      kind: "insert",
      text: "犬",
    });
    expect(editor.input("a猫犬", "insertText", false)).toBeUndefined();
  });

  it("suppresses cancellation, mismatched composition and a commit after owner reset", () => {
    const editor = new LiveEditBuffer({ textBytes: 4096, backspace: true });
    editor.compositionStart(end(0));
    expect(editor.input("n", "insertCompositionText", true)).toBeUndefined();
    expect(editor.compositionEnd("", "")).toBeUndefined();

    editor.compositionStart(end(0));
    expect(editor.compositionEnd("n", "猫")).toBeUndefined();
    expect(editor.input("犬", "insertText", false)).toEqual({ kind: "compose" });

    editor.reset();
    editor.compositionStart(end(0));
    editor.reset();
    expect(editor.compositionEnd("猫", "猫")).toBeUndefined();
    expect(editor.input("猫", "insertCompositionText", false)).toEqual({ kind: "compose" });
  });
});
