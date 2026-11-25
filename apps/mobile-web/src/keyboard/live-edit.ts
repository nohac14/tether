export interface EditSelection {
  start: number;
  end: number;
}

export type LiveAction =
  { kind: "insert"; text: string } | { kind: "delete-backward"; repeatCount: number };

export interface LiveCapabilities {
  textBytes: number;
  backspace: boolean;
}

export type EditResult = LiveAction | { kind: "compose" } | undefined;

const LOCAL_UNITS = 4096;
const HARD_TEXT_BYTES = 4096;
export const MAX_BACKSPACE_REPEAT = 64;
const appendTypes = new Set(["insertText", "insertLineBreak", "insertParagraph"]);
const backwardDeleteTypes = new Set([
  "deleteContentBackward",
  "deleteWordBackward",
  "deleteSoftLineBackward",
  "deleteHardLineBackward",
]);

export class LiveEditBuffer {
  private previous = "";
  private initialSelection: EditSelection | undefined;
  private composing = false;
  private compositionFinal: string | undefined;
  private drafting = false;
  private readonly encoder = new TextEncoder();
  private readonly capabilities: LiveCapabilities;

  constructor(capabilities: LiveCapabilities) {
    if (
      !Number.isSafeInteger(capabilities.textBytes) ||
      capabilities.textBytes < 0 ||
      capabilities.textBytes > HARD_TEXT_BYTES
    ) {
      throw new RangeError("Invalid live text byte limit");
    }
    this.capabilities = { ...capabilities };
  }

  beforeInput(selection: EditSelection): void {
    if (!this.composing && this.compositionFinal === undefined && !this.drafting) {
      this.initialSelection = selection;
    }
  }

  emptyBackspace(
    type: string,
    selection: EditSelection,
    cancelable: boolean,
  ): LiveAction | undefined {
    if (
      !backwardDeleteTypes.has(type) ||
      !cancelable ||
      !this.capabilities.backspace ||
      this.drafting ||
      this.composing ||
      this.compositionFinal !== undefined ||
      this.previous !== "" ||
      selection.start !== 0 ||
      selection.end !== 0
    )
      return undefined;
    return { kind: "delete-backward", repeatCount: 1 };
  }

  compositionStart(selection: EditSelection): void {
    this.initialSelection = selection;
    this.composing = true;
    this.compositionFinal = undefined;
  }

  compositionEnd(value: string, confirmed: string): EditResult {
    if (!this.composing || this.drafting) return undefined;
    this.composing = false;
    if (
      !this.atEnd(this.initialSelection) ||
      confirmed.length > LOCAL_UNITS - this.previous.length
    ) {
      return this.compose();
    }
    this.compositionFinal = this.previous + confirmed;
    if (value === this.compositionFinal) return this.finishComposition(value);
    return undefined;
  }

  input(value: string, type: string, isComposing: boolean, after?: EditSelection): EditResult {
    if (this.drafting || this.composing || isComposing) return undefined;
    if (this.compositionFinal !== undefined) {
      return value === this.compositionFinal ? this.finishComposition(value) : this.compose();
    }
    const before = this.initialSelection;
    const atEnd = this.atEnd(before);
    this.initialSelection = undefined;
    if (value === this.previous) return undefined;
    if (value.length > LOCAL_UNITS) return this.compose();
    if (atEnd && appendTypes.has(type) && value.startsWith(this.previous))
      return this.append(value);

    const repeatCount = this.previous.length - value.length;
    if (
      backwardDeleteTypes.has(type) &&
      this.capabilities.backspace &&
      repeatCount > 0 &&
      repeatCount <= MAX_BACKSPACE_REPEAT &&
      // A keyboard can select the word before deleting it, or deliver only
      // input. In the latter case require a collapsed caret at the new end.
      (before
        ? before.end === this.previous.length &&
          (before.start === before.end || before.start === value.length)
        : after
          ? after.start === value.length && after.end === value.length
          : repeatCount === 1) &&
      this.previous.startsWith(value) &&
      /^[\x20-\x7e]+$/.test(this.previous.slice(value.length))
    ) {
      this.previous = value;
      return { kind: "delete-backward", repeatCount };
    }
    return this.compose();
  }

  reset(): void {
    this.previous = "";
    this.initialSelection = undefined;
    this.composing = false;
    this.compositionFinal = undefined;
    this.drafting = false;
  }

  private atEnd(selection: EditSelection | undefined): boolean {
    return (
      !selection ||
      (selection.start === this.previous.length && selection.end === this.previous.length)
    );
  }

  private append(value: string): EditResult {
    const text = value.slice(this.previous.length);
    if (text === "") return undefined;
    // Iteration groups a valid surrogate pair and leaves lone code units alone.
    for (const point of text) {
      const unit = point.charCodeAt(0);
      if (point.length === 1 && unit >= 0xd800 && unit <= 0xdfff) return this.compose();
    }
    if (this.encoder.encode(text).length > this.capabilities.textBytes) return this.compose();
    this.previous = value;
    return { kind: "insert", text };
  }

  private finishComposition(value: string): EditResult {
    this.initialSelection = undefined;
    this.compositionFinal = undefined;
    return this.append(value);
  }

  private compose(): EditResult {
    this.drafting = true;
    this.compositionFinal = undefined;
    this.initialSelection = undefined;
    return { kind: "compose" };
  }
}
