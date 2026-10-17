/**
 * Whether a key belongs to IME input (Japanese conversion): the Enter that
 * confirms a conversion must not also submit the field, nor Escape that
 * cancels one clear it. macOS's WKWebView sends compositionend before that
 * keydown, so `isComposing` is already false there; keyCode 229 still marks it.
 */
export function composing(e: KeyboardEvent | React.KeyboardEvent): boolean {
  const k = "nativeEvent" in e ? e.nativeEvent : e;
  return k.isComposing || k.keyCode === 229;
}
