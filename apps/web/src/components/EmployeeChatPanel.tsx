import React from 'react'
import {
  createImeEnterGuardState,
  onCompositionEnd,
  onCompositionStart,
  shouldSubmitOnEnter,
} from '../features/employee-chat/imeSafeEnterSubmit'
import {
  useEmployeeChatMessages,
  type ChatSenderProfile,
  type ChatWireMessage,
} from '../features/employee-chat/useEmployeeChatMessages'
import { EmployeeDirectoryRecord } from './EmployeeList'

type DisplayMessage = {
  id: string
  side: 'me' | 'employee' | 'system'
  content: string
  at: string
  pending?: boolean
  senderName?: string
  senderAvatarUrl?: string
}

type EmployeeChatPanelProps = {
  apiBase: string
  locale: string
  employees: EmployeeDirectoryRecord[]
  selectedEmployeeId: string | null
  messageDraft: string
  onMessageDraftChange: (value: string) => void
  chatSenderProfile: ChatSenderProfile
  t: (key: string) => string
}

function ChatMessageAvatar(props: {
  imageUrl?: string
  label: string
  fallbackLetter: string
  altTemplate: string
}) {
  const { imageUrl, label, fallbackLetter, altTemplate } = props
  const [imgFailed, setImgFailed] = React.useState(false)
  const showImg = Boolean(imageUrl) && !imgFailed
  const ariaLabel = altTemplate.replace('{name}', label)
  const letter = fallbackLetter.trim().slice(0, 1).toUpperCase() || '?'

  return (
    <div className="chat-message__avatar" title={label} aria-label={ariaLabel}>
      {showImg ? (
        <img
          className="chat-message__avatar-img"
          src={imageUrl}
          alt=""
          onError={() => setImgFailed(true)}
        />
      ) : (
        <span className="chat-message__avatar-fallback" aria-hidden="true">
          {letter}
        </span>
      )}
    </div>
  )
}

function buildSeedMessages(
  employee: EmployeeDirectoryRecord,
  sender: ChatSenderProfile,
  t: (key: string) => string,
): DisplayMessage[] {
  return [
    {
      id: `${employee.id}-seed-1`,
      side: 'employee',
      content: t('ui.chat.seed.employeeIntro').replace('{name}', employee.name),
      at: '09:20',
    },
    {
      id: `${employee.id}-seed-2`,
      side: 'me',
      content: t('ui.chat.seed.managerReply'),
      at: '09:23',
      senderName: sender.name,
      senderAvatarUrl: sender.avatarUrl || undefined,
    },
  ]
}

function formatMessageTimeMs(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })
}

function wireToDisplay(m: ChatWireMessage): DisplayMessage {
  const side = m.role === 'user' ? 'me' : m.role === 'system' ? 'system' : 'employee'
  return {
    id: m.id,
    side,
    content: m.content,
    at: formatMessageTimeMs(m.created_at_ms),
    senderName: m.sender_name ?? undefined,
    senderAvatarUrl: m.sender_avatar_url ?? undefined,
  }
}

export function EmployeeChatPanel({
  apiBase,
  locale,
  employees,
  selectedEmployeeId,
  messageDraft,
  onMessageDraftChange,
  chatSenderProfile,
  t,
}: EmployeeChatPanelProps) {
  const selectedEmployee = employees.find((item) => item.id === selectedEmployeeId) ?? null
  const imeEnterGuardRef = React.useRef(createImeEnterGuardState())
  const { serverMessages, optimisticUser, streamingAssistantText, loading, sending, error, lastResult, sendMessage } =
    useEmployeeChatMessages(apiBase, locale, selectedEmployeeId, chatSenderProfile)

  const historyRef = React.useRef<HTMLDivElement>(null)

  const displayMessages = React.useMemo((): DisplayMessage[] => {
    if (!selectedEmployee) return []
    let base: DisplayMessage[]
    if (serverMessages.length > 0) {
      base = serverMessages.map(wireToDisplay)
    } else {
      base = buildSeedMessages(selectedEmployee, chatSenderProfile, t)
    }
    if (optimisticUser) {
      base = [...base, wireToDisplay(optimisticUser)]
    }
    if (sending && optimisticUser) {
      if (streamingAssistantText.length > 0) {
        base = [
          ...base,
          {
            id: 'stream-assistant',
            side: 'employee',
            content: streamingAssistantText,
            at: '',
            pending: true,
          },
        ]
      } else {
        base = [
          ...base,
          {
            id: 'optimistic-typing',
            side: 'employee',
            content: t('ui.chat.awaitingReply'),
            at: '',
            pending: true,
          },
        ]
      }
    }
    return base
  }, [chatSenderProfile, optimisticUser, selectedEmployee, sending, serverMessages, streamingAssistantText, t])

  React.useEffect(() => {
    const el = historyRef.current
    if (!el) return
    el.scrollTop = el.scrollHeight
  }, [displayMessages, sending])

  const sendFromDraft = React.useCallback(async () => {
    if (!selectedEmployeeId || !selectedEmployee) return
    const text = messageDraft.trim()
    if (!text) return
    onMessageDraftChange('')
    try {
      await sendMessage(text)
    } catch {
      onMessageDraftChange(text)
    }
  }, [messageDraft, onMessageDraftChange, selectedEmployee, selectedEmployeeId, sendMessage])

  const handlePromptKeyDown = React.useCallback(
    (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
      const native = event.nativeEvent
      if (
        !shouldSubmitOnEnter(
          {
            key: event.key,
            shiftKey: event.shiftKey,
            isComposing: native.isComposing,
            keyCode: native.keyCode,
          },
          imeEnterGuardRef.current,
        )
      ) {
        return
      }
      event.preventDefault()
      void sendFromDraft()
    },
    [sendFromDraft],
  )

  const messageClass = (message: DisplayMessage) => {
    if (message.pending) return 'chat-message chat-message--employee chat-message--pending'
    if (message.side === 'me') return 'chat-message chat-message--me'
    if (message.side === 'system') return 'chat-message chat-message--system'
    return 'chat-message chat-message--employee'
  }

  const defaultSenderName = t('ui.chat.senderDefaultName')
  const avatarAlt = t('ui.chat.avatarAlt')

  return (
    <div className="chat-layout">
      {selectedEmployee ? (
        <>
          <div className="chat-history" ref={historyRef}>
            {loading ? <div className="chat-history__status">{t('ui.chat.loadingHistory')}</div> : null}
            {!loading && lastResult && lastResult.exit_code !== 0 ? (
              <div className="chat-history__warn">
                {t('ui.chat.toolExitWarning').replace('{code}', String(lastResult.exit_code))}
              </div>
            ) : null}
            {displayMessages.map((message) => {
              const isMe = message.side === 'me'
              const isSystem = message.side === 'system'
              const userLabel = message.senderName?.trim() || defaultSenderName
              const employeeLabel = selectedEmployee.name
              const systemLabel = t('ui.chat.systemSenderLabel')
              const peerSecondary = isSystem ? '' : `${selectedEmployee.department} / ${selectedEmployee.role}`

              const bubble = (
                <div className="chat-message__bubble">
                  <div className="chat-message__content">{message.content}</div>
                </div>
              )

              const stamp =
                message.at.trim().length > 0 ? (
                  <div className="chat-message__stamp">{message.at}</div>
                ) : null

              return (
                <div key={message.id} className={messageClass(message)}>
                  {isMe ? (
                    <div className="chat-message__stack chat-message__stack--me">
                      <div className="chat-message__head chat-message__head--me">
                        <div className="chat-message__sender">
                          <div className="chat-message__sender-line1">{userLabel}</div>
                          <div className="chat-message__sender-line2">{t('ui.chat.userSenderCaption')}</div>
                        </div>
                        <ChatMessageAvatar
                          imageUrl={message.senderAvatarUrl}
                          label={userLabel}
                          fallbackLetter={userLabel}
                          altTemplate={avatarAlt}
                        />
                      </div>
                      {bubble}
                      {stamp}
                    </div>
                  ) : (
                    <div className="chat-message__stack chat-message__stack--peer">
                      <div className="chat-message__head chat-message__head--peer">
                        <ChatMessageAvatar
                          label={isSystem ? systemLabel : employeeLabel}
                          fallbackLetter={isSystem ? '!' : employeeLabel}
                          altTemplate={avatarAlt}
                        />
                        <div className="chat-message__sender">
                          <div className="chat-message__sender-line1">
                            {isSystem ? systemLabel : employeeLabel}
                          </div>
                          {peerSecondary ? (
                            <div className="chat-message__sender-line2">{peerSecondary}</div>
                          ) : null}
                        </div>
                      </div>
                      {bubble}
                      {stamp}
                    </div>
                  )}
                </div>
              )
            })}
          </div>
          {!loading && error ? (
            <div className="chat-inline-status chat-inline-status--error">
              {error}
            </div>
          ) : null}
          <div className="chat-input-wrap">
            <div className="chat-toolbar chat-toolbar--top">
              <button type="button" className="action-btn">
                {t('ui.chat.toolbar.attach')}
              </button>
              <button type="button" className="action-btn">
                {t('ui.chat.toolbar.template')}
              </button>
            </div>
            <textarea
              className="chat-input"
              value={messageDraft}
              onChange={(event) => onMessageDraftChange(event.target.value)}
              onCompositionStart={() => onCompositionStart(imeEnterGuardRef.current)}
              onCompositionEnd={() => onCompositionEnd(imeEnterGuardRef.current)}
              onKeyDown={handlePromptKeyDown}
              disabled={sending}
              placeholder={t('ui.chat.placeholder').replace('{name}', selectedEmployee.name)}
            />
            <div className="chat-toolbar chat-toolbar--bottom">
              <button type="button" className="action-btn" disabled={sending}>
                {t('ui.chat.toolbar.emoji')}
              </button>
              <button type="button" className="action-btn" onClick={() => void sendFromDraft()} disabled={sending}>
                {t('ui.chat.toolbar.send')}
              </button>
            </div>
          </div>
        </>
      ) : (
        <div className="content-placeholder">
          <div>{t('ui.chat.emptySelection')}</div>
        </div>
      )}
    </div>
  )
}
