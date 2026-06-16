import React from 'react'
import {
  CHAT_TEXT_PREVIEW_MAX,
  chatTextPreview,
  hiddenChatTextCount,
  shouldCollapseChatText,
} from '../features/employee-chat/collapsibleMessageText'

type CollapsibleMessageTextProps = {
  text: string
  className?: string
  maxChars?: number
  /** When true the full text is always shown (live streaming). */
  live?: boolean
  t: (key: string) => string
}

export const CollapsibleMessageText = React.memo(function CollapsibleMessageText({
  text,
  className,
  maxChars = CHAT_TEXT_PREVIEW_MAX,
  live = false,
  t,
}: CollapsibleMessageTextProps) {
  const [expanded, setExpanded] = React.useState(false)
  const canCollapse = !live && shouldCollapseChatText(text, maxChars)
  const showFull = !canCollapse || expanded

  if (showFull) {
    return (
      <div className={className}>
        {text}
        {canCollapse ? (
          <button
            type="button"
            className="chat-text-fold__toggle"
            onClick={() => setExpanded(false)}
          >
            {t('ui.chat.text.collapse')}
          </button>
        ) : null}
      </div>
    )
  }

  const hidden = hiddenChatTextCount(text, maxChars)
  return (
    <div className={className}>
      {chatTextPreview(text, maxChars)}
      <span className="chat-text-fold__ellipsis" aria-hidden="true">
        …
      </span>
      <button
        type="button"
        className="chat-text-fold__toggle"
        onClick={() => setExpanded(true)}
      >
        {t('ui.chat.text.expand').replace('{count}', String(hidden))}
      </button>
    </div>
  )
})
