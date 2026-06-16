import { describe, expect, it } from 'vitest'
import {
  CHAT_TEXT_PREVIEW_MAX,
  chatTextPreview,
  hiddenChatTextCount,
  shouldCollapseChatText,
} from './collapsibleMessageText'

describe('collapsibleMessageText', () => {
  it('does not collapse short text', () => {
    const text = 'hello world'
    expect(shouldCollapseChatText(text)).toBe(false)
    expect(chatTextPreview(text)).toBe(text)
    expect(hiddenChatTextCount(text)).toBe(0)
  })

  it('collapses long text and prefers breaking at a newline', () => {
    const head = 'a'.repeat(CHAT_TEXT_PREVIEW_MAX - 20)
    const text = `${head}\n${'b'.repeat(200)}`
    expect(shouldCollapseChatText(text)).toBe(true)
    expect(chatTextPreview(text)).toBe(head)
    expect(hiddenChatTextCount(text)).toBe(201)
  })
})
