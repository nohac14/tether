import {
  LiveEditBuffer,
  type EditResult,
  type LiveAction,
  type LiveCapabilities,
} from "./live-edit";

export interface KeyboardBinding extends LiveCapabilities {
  send(action: LiveAction): void;
}

/** Persistent editor. A new owner always starts with an empty phone buffer. */
export function mountKeyboard(root: HTMLElement): {
  setOwner(owner?: KeyboardBinding): void;
  dispose(): void;
} {
  const button = root.querySelector<HTMLButtonElement>("#keyboard-open");
  const editor = root.querySelector<HTMLTextAreaElement>("#keyboard-editor");
  const feedback = root.querySelector<HTMLElement>("#keyboard-feedback");
  const document = root.ownerDocument;
  const bufferLimit = editor?.maxLength ?? 4096;
  const listeners = new AbortController();
  const signal = { signal: listeners.signal };
  let owner: KeyboardBinding | undefined;
  let engine: LiveEditBuffer | undefined;
  // Give the software keyboard something to erase even after the last local
  // character. Some keyboards omit edit events for an already-empty field.
  const marker = "\u200b";
  let marked = false;
  const message = (text: string) => {
    if (feedback) {
      feedback.textContent = text;
      feedback.hidden = text.length === 0;
    }
  };
  const reset = () => {
    engine?.reset();
    marked = false;
    if (editor) {
      editor.value = "";
      editor.maxLength = bufferLimit;
      delete editor.dataset.local;
      if (button) button.textContent = "Keyboard";
    }
    message("");
  };
  if (editor && button) {
    const focused = () => owner !== undefined && document.activeElement === editor;
    const prefix = () => Number(marked && editor.value.startsWith(marker));
    const selection = () => ({
      start: Math.max(0, editor.selectionStart - prefix()),
      end: Math.max(0, editor.selectionEnd - prefix()),
    });
    const prime = () => {
      if (editor.value === "" && !editor.dataset.local && owner?.backspace) {
        editor.value = marker;
        editor.maxLength = bufferLimit + 1;
        editor.setSelectionRange(1, 1);
        marked = true;
      }
    };
    const stripMarker = () => {
      if (prefix()) {
        const { start, end } = selection();
        editor.value = editor.value.slice(1);
        editor.setSelectionRange(start, end);
      }
      marked = false;
      editor.maxLength = bufferLimit;
    };
    const deliver = (result: EditResult) => {
      if (!result || !focused()) return;
      if (result.kind === "compose") {
        stripMarker();
        editor.dataset.local = "true";
        button.textContent = "Resume live typing";
        message("Editing locally. Resume live typing to discard this draft.");
      } else {
        owner?.send(result);
      }
    };
    button.addEventListener(
      "click",
      () => {
        if (!owner) return;
        if (editor.dataset.local) reset();
        editor.focus({ preventScroll: true });
        if (!focused()) message("Tap Keyboard again to open it.");
        else {
          prime();
          message("");
        }
      },
      signal,
    );
    editor.addEventListener(
      "beforeinput",
      (event) => {
        if (!focused() || !(event instanceof InputEvent) || event.isComposing) return;
        const selected = selection();
        const action = engine?.emptyBackspace(
          event.inputType,
          selected,
          editor.value.length === prefix() && event.cancelable,
        );
        if (action) {
          event.preventDefault();
          deliver(action);
        } else {
          engine?.beforeInput(selected);
        }
      },
      signal,
    );
    editor.addEventListener(
      "input",
      (event) => {
        if (focused() && event instanceof InputEvent) {
          if (
            editor.dataset.local &&
            editor.value === "" &&
            event.inputType.startsWith("delete") &&
            !event.isComposing
          ) {
            reset();
            prime();
            return;
          }
          const value = editor.value.slice(prefix());
          if (marked && editor.value === "" && !event.isComposing) {
            // The browser removed only the placeholder in a noncancelable
            // edit. Its completed mutation confirms one backward intention.
            deliver(engine?.emptyBackspace(event.inputType, selection(), true));
          }
          const edit = engine?.input(value, event.inputType, event.isComposing, selection());
          deliver(edit);
          if (!event.isComposing && !editor.dataset.local) {
            if (edit?.kind === "insert") stripMarker();
            if (value === "" && event.inputType.startsWith("delete")) prime();
          }
        }
      },
      signal,
    );
    editor.addEventListener(
      "compositionstart",
      () => {
        if (focused()) engine?.compositionStart(selection());
      },
      signal,
    );
    editor.addEventListener(
      "compositionend",
      (event) => {
        if (focused()) deliver(engine?.compositionEnd(editor.value.slice(prefix()), event.data));
      },
      signal,
    );
    editor.addEventListener(
      "keydown",
      (event) => {
        if (event.key === "Escape" && !event.isComposing) {
          event.preventDefault();
          editor.blur();
        }
      },
      signal,
    );
    editor.addEventListener(
      "blur",
      () => {
        if (!editor.dataset.local) reset();
      },
      signal,
    );
    document.addEventListener(
      "visibilitychange",
      () => {
        if (document.hidden) {
          editor.blur();
          reset();
        }
      },
      signal,
    );
    document.defaultView?.addEventListener(
      "pagehide",
      () => {
        editor.blur();
        reset();
      },
      signal,
    );
  }
  const setOwner = (binding?: KeyboardBinding) => {
    if (owner === binding) return;
    const next = binding && (binding.textBytes > 0 || binding.backspace) ? binding : undefined;
    const nextEngine = next && new LiveEditBuffer(next);
    editor?.blur();
    reset();
    owner = next;
    engine = nextEngine;
    if (editor) editor.disabled = !owner;
    if (button) button.disabled = !owner;
  };
  return {
    setOwner,
    dispose() {
      setOwner();
      listeners.abort();
    },
  };
}
