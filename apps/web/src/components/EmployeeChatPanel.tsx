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
  type ChatResultMeta,
  type StreamingAssistantState,
  type StreamingToolCall,
  type StreamingBlock,
  reconstructStreamingState,
} from '../features/employee-chat/useEmployeeChatMessages'
import { EmployeeDirectoryRecord } from './EmployeeList'

type StreamingExtras = {
  thinking: string
  toolCalls: StreamingToolCall[]
  session: StreamingAssistantState['session']
  result: StreamingAssistantState['result']
  /** Ordered blocks for section-based rendering. */
  blocks: StreamingBlock[]
}

type DisplayMessage = {
  id: string
  side: 'me' | 'employee' | 'system'
  content: string
  at: string
  pending?: boolean
  senderName?: string
  senderAvatarUrl?: string
  streaming?: StreamingExtras
  taskResult?: ChatResultMeta
}

type EmployeeChatPanelProps = {
  apiBase: string
  locale: string
  employees: EmployeeDirectoryRecord[]
  selectedEmployeeId: string | null
  messageDraft: string
  onMessageDraftChange: (value: string) => void
  chatSenderProfile: ChatSenderProfile
  onEmployeeTasksRefresh?: () => void
  /** When this counter changes, the chat panel will reload messages. */
  chatMessagesRefreshTick?: number
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
  const base: DisplayMessage = {
    id: m.id,
    side,
    content: m.content,
    at: formatMessageTimeMs(m.created_at_ms),
    senderName: m.sender_name ?? undefined,
    senderAvatarUrl: m.sender_avatar_url ?? undefined,
  }

  // Handle task_process messages: reconstruct streaming state from saved events
  if (m.role === 'task_process' && m.stream_events && m.stream_events.length > 0) {
    const streamingState = reconstructStreamingState(m.stream_events)
    const streamingExtras: StreamingExtras = {
      thinking: streamingState.thinking,
      toolCalls: streamingState.toolCalls,
      session: streamingState.session,
      result: streamingState.result,
      blocks: streamingState.blocks,
    }
    return {
      ...base,
      content: m.content || streamingState.text,
      pending: m.task_status === 'running',
      streaming: streamingState.blocks.length > 0 || streamingState.text.length > 0 ? streamingExtras : undefined,
      taskResult: m.result_meta ?? undefined,
    }
  }

  return base
}

function StreamingToolCallCard({
  call,
  t,
}: {
  call: StreamingToolCall
  t: (key: string) => string
}) {
  const statusKey =
    call.status === 'running'
      ? 'ui.chat.streaming.toolStatusRunning'
      : call.status === 'error'
        ? 'ui.chat.streaming.toolStatusError'
        : 'ui.chat.streaming.toolStatusSuccess'
  return (
    <div className={`stream-tool-call stream-tool-call--${call.status}`}>
      <div className="stream-tool-call__header">
        <span className="stream-tool-call__name">{call.name || t('ui.chat.streaming.unknownTool')}</span>
        <span className="stream-tool-call__status">{t(statusKey)}</span>
      </div>
      {call.inputSummary ? (
        <details className="stream-tool-call__details">
          <summary className="stream-tool-call__summary">{t('ui.chat.streaming.input')}</summary>
          <pre className="stream-tool-call__pre">{call.inputSummary}</pre>
        </details>
      ) : null}
      {call.outputPreview ? (
        <details className="stream-tool-call__details" open={call.status === 'error'}>
          <summary className="stream-tool-call__summary">{t('ui.chat.streaming.output')}</summary>
          <pre className="stream-tool-call__pre">{call.outputPreview}</pre>
        </details>
      ) : null}
    </div>
  )
}

function TaskResultPanel({ result, t }: { result: ChatResultMeta | null; t: (key: string) => string }) {
  if (!result) return null
  const isSuccess = result.exit_code === 0
  return (
    <div className={`task-result-panel task-result-panel--${isSuccess ? 'success' : 'error'}`}>
      <div className="task-result-panel__header">
        <span className="task-result-panel__title">{t('ui.chat.taskResult.title')}</span>
        <span className={`task-result-panel__status task-result-panel__status--${isSuccess ? 'success' : 'error'}`}>
          {isSuccess ? t('ui.chat.taskResult.success') : t('ui.chat.taskResult.failed')}
        </span>
      </div>
      <div className="task-result-panel__meta">
        <span className="task-result-panel__tool">{t('ui.chat.taskResult.tool')}: {result.tool_kind}</span>
        <span className="task-result-panel__tokens">
          {t('ui.chat.taskResult.tokens')}: {result.prompt_tokens.toLocaleString()} / {result.completion_tokens.toLocaleString()}
        </span>
      </div>
      {result.output_preview && (
        <details className="task-result-panel__details">
          <summary className="task-result-panel__summary">{t('ui.chat.taskResult.outputPreview')}</summary>
          <pre className="task-result-panel__output">{result.output_preview}</pre>
        </details>
      )}
    </div>
  )
}

export function EmployeeChatPanel({
  apiBase,
  locale,
  employees,
  selectedEmployeeId,
  messageDraft,
  onMessageDraftChange,
  chatSenderProfile,
  onEmployeeTasksRefresh,
  chatMessagesRefreshTick,
  t,
}: EmployeeChatPanelProps) {
  const selectedEmployee = employees.find((item) => item.id === selectedEmployeeId) ?? null
  const imeEnterGuardRef = React.useRef(createImeEnterGuardState())
  const { serverMessages, optimisticUser, streamingAssistant, loading, sending, error, lastResult, refresh, sendMessage } =
    useEmployeeChatMessages(apiBase, locale, selectedEmployeeId, chatSenderProfile, onEmployeeTasksRefresh)

  // Refresh messages when parent signals a task completed
  React.useEffect(() => {
    if (chatMessagesRefreshTick) {
      refresh()
    }
  }, [chatMessagesRefreshTick, refresh])

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
      const streamingExtras: StreamingExtras = {
        thinking: streamingAssistant.thinking,
        toolCalls: streamingAssistant.toolCalls,
        session: streamingAssistant.session,
        result: streamingAssistant.result,
        blocks: streamingAssistant.blocks,
      }
      const hasAnyContent =
        streamingAssistant.blocks.length > 0 ||
        streamingAssistant.text.length > 0 ||
        streamingAssistant.thinking.length > 0 ||
        streamingAssistant.toolCalls.length > 0 ||
        streamingAssistant.session !== null ||
        streamingAssistant.result !== null
      if (hasAnyContent) {
        base = [
          ...base,
          {
            id: 'stream-assistant',
            side: 'employee',
            content: streamingAssistant.text,
            at: '',
            pending: true,
            streaming: streamingExtras,
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
  }, [chatSenderProfile, optimisticUser, selectedEmployee, sending, serverMessages, streamingAssistant, t])

  React.useEffect(() => {
    const el = historyRef.current
    if (!el) return
    el.scrollTop = el.scrollHeight
  }, [displayMessages, sending, streamingAssistant])

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
            {!loading && lastResult ? (
              <TaskResultPanel result={lastResult} t={t} />
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
                  {message.streaming ? (
                    <>
                      {message.streaming.session ? (
                        <div className="stream-progress__session">
                          <span className="stream-progress__chip">
                            {t('ui.chat.streaming.sessionStarted')}
                          </span>
                          {message.streaming.session.model ? (
                            <span className="stream-progress__meta">{message.streaming.session.model}</span>
                          ) : null}
                          {message.streaming.session.tools.length > 0 ? (
                            <span className="stream-progress__meta">
                              {t('ui.chat.streaming.tools')}: {message.streaming.session.tools.length}
                            </span>
                          ) : null}
                        </div>
                      ) : null}
                      <div className="streaming-sections">
                        {message.streaming.blocks.map((block, blockIdx) => {
                          if (block.type === 'text') {
                            return (
                              <div className="streaming-section streaming-section--text" key={`b-${blockIdx}`}>
                                <div className="chat-message__content">{block.text}</div>
                              </div>
                            )
                          }
                          if (block.type === 'thinking') {
                            return (
                              <div className="streaming-section streaming-section--thinking" key={`b-${blockIdx}`}>
                                <details open>
                                  <summary className="stream-progress__summary">{t('ui.chat.streaming.thinking')}</summary>
                                  <pre className="stream-progress__pre">{block.text}</pre>
                                </details>
                              </div>
                            )
                          }
                          if (block.type === 'tool_call') {
                            return <StreamingToolCallCard key={block.id} call={block} t={t} />
                          }
                          if (block.type === 'result') {
                            return (
                              <div
                                className={`stream-progress__result stream-progress__result--${block.isError ? 'error' : 'success'}`}
                                key={`b-${blockIdx}`}
                              >
                                <span className="stream-progress__chip">
                                  {block.isError
                                    ? t('ui.chat.streaming.resultError')
                                    : t('ui.chat.streaming.resultSuccess')}
                                </span>
                                <span className="stream-progress__meta">
                                  {t('ui.chat.taskResult.tokens')}: {block.promptTokens.toLocaleString()} / {block.completionTokens.toLocaleString()}
                                </span>
                              </div>
                            )
                          }
                          return null
                        })}
                        {!message.streaming.blocks.length && !message.content ? (
                          <div className="chat-message__content chat-message__content--placeholder">
                            {t('ui.chat.awaitingReply')}
                          </div>
                        ) : null}
                      </div>
                    </>
                  ) : null}
                  {message.content && !message.streaming ? (
                    <div className="chat-message__content">{message.content}</div>
                  ) : null}
                  {message.streaming && !message.content && !message.streaming.blocks.length ? (
                    <div className="chat-message__content chat-message__content--placeholder">
                      {t('ui.chat.awaitingReply')}
                    </div>
                  ) : null}
                  {message.taskResult ? (
                    <TaskResultPanel result={message.taskResult} t={t} />
                  ) : null}
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
