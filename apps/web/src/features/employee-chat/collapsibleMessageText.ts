/** Default visible character budget before folding long chat text. */
export const CHAT_TEXT_PREVIEW_MAX = 480

export function shouldCollapseChatText(text: string, maxChars = CHAT_TEXT_PREVIEW_MAX): boolean {
  return text.length > maxChars
}

/** Returns a prefix of `text` suitable for the collapsed preview. */
export function chatTextPreview(text: string, maxChars = CHAT_TEXT_PREVIEW_MAX): string {
  if (text.length <= maxChars) return text
  const window = text.slice(0, maxChars)
  const lastBreak = Math.max(window.lastIndexOf('\n'), window.lastIndexOf(' '))
  if (lastBreak > maxChars * 0.6) {
    return window.slice(0, lastBreak).trimEnd()
  }
  return window.trimEnd()
}

export function hiddenChatTextCount(text: string, maxChars = CHAT_TEXT_PREVIEW_MAX): number {
  if (text.length <= maxChars) return 0
  return text.length - chatTextPreview(text, maxChars).length
}
