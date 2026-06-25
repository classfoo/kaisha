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
import { ChatHistoryList, type ChatHistoryListHandle } from './ChatHistoryList'
import { CollapsibleMessageText } from './CollapsibleMessageText'

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
  taskId?: string | null
  taskStatus?: string | null
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

export const CopyButton = React.memo(function CopyButton({ text, t }: { text: string; t: (key: string) => string }) {
  const [copied, setCopied] = React.useState(false)
  const handleCopy = React.useCallback(() => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    })
  }, [text])
  return (
    <button
      type="button"
      className="chat-message__copy-btn"
      onClick={handleCopy}
      title={copied ? t('ui.chat.copy.copied') : t('ui.chat.copy.title')}
      aria-label={copied ? t('ui.chat.copy.copied') : t('ui.chat.copy.title')}
    >
      {copied ? t('ui.chat.copy.checkmark') : t('ui.chat.copy.icon')}
    </button>
  )
})

function extractCopyableContent(message: DisplayMessage): string {
  const parts: string[] = []
  if (message.content) parts.push(message.content)
  if (message.streaming) {
    for (const block of message.streaming.blocks) {
      if (block.type === 'text') parts.push(block.text)
      else if (block.type === 'thinking') parts.push(block.text)
      else if (block.type === 'tool_call') {
        if (block.inputSummary) parts.push(`[${block.name} input]\n${block.inputSummary}`)
        if (block.outputPreview) parts.push(`[${block.name} output]\n${block.outputPreview}`)
      }
    }
  }
  return parts.join('\n\n')
}

function MessageCopyButton({ message, t }: { message: DisplayMessage; t: (key: string) => string }) {
  const content = extractCopyableContent(message)
  if (!content) return null
  return (
    <div className="chat-message__bubbleActions">
      <CopyButton text={content} t={t} />
    </div>
  )
}

function TypingIndicator() {
  return (
    <div className="typing-indicator">
      <span className="typing-dot" style={{ '--i': 0 } as React.CSSProperties}>.</span>
      <span className="typing-dot" style={{ '--i': 1 } as React.CSSProperties}>.</span>
      <span className="typing-dot" style={{ '--i': 2 } as React.CSSProperties}>.</span>
    </div>
  )
}

export const TaskCrashedPanel = React.memo(function TaskCrashedPanel({
  taskId,
  continuing,
  onContinue,
  t,
}: {
  taskId: string
  continuing: boolean
  onContinue: (taskId: string) => void
  t: (key: string) => string
}) {
  return (
    <div className="task-crashed-panel">
      <div className="task-crashed-panel__message">{t('ui.chat.processCrashed')}</div>
      <button
        type="button"
        className="task-crashed-panel__button"
        onClick={() => onContinue(taskId)}
        disabled={continuing}
      >
        {continuing ? t('ui.chat.continuing') : t('ui.chat.continueExecution')}
      </button>
    </div>
  )
})

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

export function wireToDisplay(m: ChatWireMessage): DisplayMessage {
  return _wireToDisplayImpl(m)
}

// Module-level cache: avoids recomputing the (potentially O(n)) streaming-state
// reconstruction for messages whose rendered content has not changed.
//
// IMPORTANT: a `task_process` message keeps the same `id` while its
// `stream_events` grow during a live code-agent run (the conversation watch SSE
// relays incremental updates for that same message). The cache key therefore
// MUST incorporate a content signature, not just the id — otherwise the first
// streamed chunk is cached forever and the real-time process never updates.
const _wireCache = new Map<string, { signature: string; result: DisplayMessage }>()

/**
 * Cheap signature capturing every field `_computeWireToDisplay` depends on that
 * can change for a given message id across SSE updates. `stream_events` are
 * append-only on the backend, so their length tracks incremental growth;
 * `task_status`/`content`/`result_meta` change when a run finalizes.
 */
function streamEventsTextLen(m: ChatWireMessage): number {
  if (!m.stream_events?.length) return 0
  let len = 0
  for (const ev of m.stream_events) {
    if (ev && typeof ev === 'object' && 'text' in ev && typeof ev.text === 'string') {
      len += ev.text.length
    }
  }
  return len
}

function _wireSignature(m: ChatWireMessage): string {
  const events = m.stream_events?.length ?? 0
  return `${events}|${streamEventsTextLen(m)}|${m.task_status ?? ''}|${m.content?.length ?? 0}|${m.result_meta ? 1 : 0}`
}

function _wireToDisplayImpl(m: ChatWireMessage): DisplayMessage {
  const signature = _wireSignature(m)
  const cached = _wireCache.get(m.id)
  if (cached && cached.signature === signature) {
    return cached.result
  }
  const result = _computeWireToDisplay(m)
  _wireCache.set(m.id, { signature, result })
  return result
}

function _computeWireToDisplay(m: ChatWireMessage): DisplayMessage {
  const side = m.role === 'user' ? 'me' : m.role === 'system' ? 'system' : 'employee'
  const base: DisplayMessage = {
    id: m.id,
    side,
    content: m.content,
    at: formatMessageTimeMs(m.created_at_ms),
    senderName: m.sender_name ?? undefined,
    senderAvatarUrl: m.sender_avatar_url ?? undefined,
    taskId: m.task_id ?? undefined,
  }

  if (m.role === 'task_process') {
    const taskStatus = m.task_status ?? undefined

    if (m.stream_events && m.stream_events.length > 0) {
      const streamingState = reconstructStreamingState(m.stream_events)
      const streamingExtras: StreamingExtras = {
        thinking: streamingState.thinking,
        toolCalls: streamingState.toolCalls,
        session: streamingState.session,
        result: streamingState.result,
        blocks: streamingState.blocks,
      }
      const hasStreamingContent =
        streamingState.blocks.length > 0 || streamingState.text.length > 0
      return {
        ...base,
        content: m.content || streamingState.text,
        pending: taskStatus === 'running',
        streaming: hasStreamingContent ? streamingExtras : undefined,
        taskResult: m.result_meta ?? undefined,
        taskStatus,
      }
    }

    return {
      ...base,
      content: m.content,
      pending: taskStatus === 'running',
      taskStatus,
    }
  }

  return base
}

export const StreamingToolCallCard = React.memo(function StreamingToolCallCard({
  call,
  t,
}: {
  call: StreamingToolCall
  t: (key: string) => string
}) {
  const [expanded, setExpanded] = React.useState(false)
  const statusKey =
    call.status === 'running'
      ? 'ui.chat.streaming.toolStatusRunning'
      : call.status === 'error'
        ? 'ui.chat.streaming.toolStatusError'
        : 'ui.chat.streaming.toolStatusSuccess'

  const toolName = call.name || t('ui.chat.streaming.unknownTool')
  const toolNameLower = toolName.toLowerCase()
  const todoItems = toolNameLower === 'todowrite' ? parseTodoItems(call.inputSummary) : []
  const summaryHint =
    todoItems.length > 0
      ? t('ui.chat.streaming.todoItemCount').replace('{count}', String(todoItems.length))
      : call.outputPreview
        ? t('ui.chat.streaming.hasOutput')
        : call.inputSummary
          ? t('ui.chat.streaming.hasInput')
          : null

  return (
    <div className={`stream-tool-call stream-tool-call--${call.status}${toolNameLower === 'todowrite' ? ' stream-tool-call--todowrite' : ''}`}>
      <details
        className="stream-tool-call__details"
        onToggle={(event) => setExpanded(event.currentTarget.open)}
      >
        <summary className="stream-tool-call__summary">
          <span className="stream-tool-call__header">
            <span className="stream-tool-call__name">{toolName}</span>
            <span className="stream-tool-call__status">{t(statusKey)}</span>
          </span>
          {summaryHint ? <span className="stream-tool-call__hint">{summaryHint}</span> : null}
        </summary>
        {expanded ? <StreamingToolCallBody call={call} todoItems={todoItems} t={t} /> : null}
      </details>
    </div>
  )
})

const StreamingToolCallBody = React.memo(function StreamingToolCallBody({
  call,
  todoItems,
  t,
}: {
  call: StreamingToolCall
  todoItems: TodoItem[]
  t: (key: string) => string
}) {
  const toolNameLower = (call.name || '').toLowerCase()

  if (toolNameLower === 'todowrite') {
    return (
      <>
        {todoItems.length > 0 ? (
          <div className="stream-tool-call__todo-list">
            {todoItems.map((todo, idx) => (
              <div key={idx} className={`stream-tool-call__todo-item${todo.done ? ' stream-tool-call__todo-item--done' : ''}`}>
                <span className="stream-tool-call__todo-checkbox">{todo.done ? '☑' : '☐'}</span>
                <span className="stream-tool-call__todo-text">{todo.text}</span>
              </div>
            ))}
          </div>
        ) : call.inputSummary ? (
          <CollapsibleMessageText className="stream-tool-call__pre" text={call.inputSummary} t={t} />
        ) : null}
        {call.outputPreview ? (
          <div className="stream-tool-call__inline-section">
            <span className="stream-tool-call__label">{t('ui.chat.streaming.output')}</span>
            <CollapsibleMessageText className="stream-tool-call__pre" text={call.outputPreview} t={t} />
          </div>
        ) : null}
      </>
    )
  }

  return (
    <>
      {call.inputSummary ? (
        <div className="stream-tool-call__inline-section">
          <span className="stream-tool-call__label">{t('ui.chat.streaming.input')}</span>
          <CollapsibleMessageText className="stream-tool-call__pre" text={call.inputSummary} t={t} />
        </div>
      ) : null}
      {call.outputPreview ? (
        <div className="stream-tool-call__inline-section">
          <span className="stream-tool-call__label">{t('ui.chat.streaming.output')}</span>
          <CollapsibleMessageText className="stream-tool-call__pre" text={call.outputPreview} t={t} />
        </div>
      ) : null}
    </>
  )
})

type TodoItem = { text: string; done: boolean }

function parseTodoItems(raw: string): TodoItem[] {
  const items: TodoItem[] = []
  try {
    // Try parsing as JSON first (most common format from Claude Code)
    const parsed = JSON.parse(raw)
    if (Array.isArray(parsed)) {
      for (const entry of parsed) {
        if (entry && typeof entry === 'object' && 'content' in entry) {
          items.push({ text: String(entry.content), done: Boolean(entry.done) })
        } else if (entry && typeof entry === 'object' && 'text' in entry) {
          items.push({ text: String(entry.text), done: Boolean(entry.done) })
        }
      }
      if (items.length > 0) return items
    }
  } catch {
    // not JSON, try line-by-line parsing
  }

  // Fall back to line-based parsing
  const lines = raw.split('\n').filter((l) => l.trim())
  for (const line of lines) {
    const trimmed = line.trim()
    if (trimmed.startsWith('- [x]') || trimmed.startsWith('- [X]')) {
      items.push({ text: trimmed.replace(/^- \[[xX]\]\s*/, '').trim(), done: true })
    } else if (trimmed.startsWith('- [ ]')) {
      items.push({ text: trimmed.replace(/^- \[ \]\s*/, '').trim(), done: false })
    } else if (trimmed.startsWith('- ')) {
      items.push({ text: trimmed.replace(/^- /, '').trim(), done: false })
    } else {
      items.push({ text: trimmed, done: false })
    }
  }
  return items
}

export const TaskResultPanel = React.memo(function TaskResultPanel({ result, t }: { result: ChatResultMeta | null; t: (key: string) => string }) {
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
          <CollapsibleMessageText
            className="task-result-panel__output"
            text={result.output_preview}
            t={t}
          />
        </details>
      )}
    </div>
  )
})

function messageClass(message: DisplayMessage): string {
  if (message.pending) return 'chat-message chat-message--employee chat-message--pending'
  if (message.side === 'me') return 'chat-message chat-message--me'
  if (message.side === 'system') return 'chat-message chat-message--system'
  return 'chat-message chat-message--employee'
}

type ChatMessageItemProps = {
  message: DisplayMessage
  employeeName: string
  employeeDepartment: string
  employeeRole: string
  defaultSenderName: string
  systemLabel: string
  avatarAlt: string
  isProcessAlive: (taskId: string) => boolean
  continuingTaskIds: Set<string>
  onContinueTask: (taskId: string) => void
  t: (key: string) => string
}

/** Renders a single chat row. Memoized so that, combined with virtualization,
 *  only the small window of on-screen messages re-renders while streaming or
 *  scrolling through a large history. */
const ChatMessageItem = React.memo(function ChatMessageItem({
  message,
  employeeName,
  employeeDepartment,
  employeeRole,
  defaultSenderName,
  systemLabel,
  avatarAlt,
  isProcessAlive,
  continuingTaskIds,
  onContinueTask,
  t,
}: ChatMessageItemProps) {
  const isMe = message.side === 'me'
  const isSystem = message.side === 'system'
  const userLabel = message.senderName?.trim() || defaultSenderName
  const employeeLabel = employeeName
  const peerSecondary = isSystem ? '' : `${employeeDepartment} / ${employeeRole}`

  const showsErrorInBlocks = Boolean(
    message.streaming?.blocks.some(
      (block) => block.type === 'result' && block.isError && Boolean(block.summary),
    ),
  )

  const bubble = (
    <div className="chat-message__bubble">
      <MessageCopyButton message={message} t={t} />
      {message.streaming ? (
        <>
          {message.streaming.session ? (
            <div className="stream-progress__session">
              <span className="stream-progress__chip">{t('ui.chat.streaming.sessionStarted')}</span>
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
                    <CollapsibleMessageText
                      className="chat-message__content"
                      text={block.text}
                      live={Boolean(message.pending)}
                      t={t}
                    />
                  </div>
                )
              }
              if (block.type === 'thinking') {
                return (
                  <div className="streaming-section streaming-section--thinking" key={`b-${blockIdx}`}>
                    <details open>
                      <summary className="stream-progress__summary">{t('ui.chat.streaming.thinking')}</summary>
                      <CollapsibleMessageText
                        className="stream-progress__pre"
                        text={block.text}
                        live={Boolean(message.pending)}
                        t={t}
                      />
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
                      {block.isError ? t('ui.chat.streaming.resultError') : t('ui.chat.streaming.resultSuccess')}
                    </span>
                    {block.summary ? (
                      <CollapsibleMessageText
                        className="stream-progress__error-detail"
                        text={block.summary}
                        t={t}
                      />
                    ) : null}
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
        <CollapsibleMessageText className="chat-message__content" text={message.content} t={t} />
      ) : null}
      {message.taskStatus === 'failed' && message.content && !showsErrorInBlocks ? (
        <CollapsibleMessageText
          className="stream-progress__result stream-progress__result--error stream-progress__error-detail"
          text={message.content}
          t={t}
        />
      ) : null}
      {message.streaming && !message.content && !message.streaming.blocks.length ? (
        <div className="chat-message__content chat-message__content--placeholder">
          {t('ui.chat.awaitingReply')}
        </div>
      ) : null}
      {message.taskResult ? <TaskResultPanel result={message.taskResult} t={t} /> : null}
      {message.pending && message.taskId && isProcessAlive(message.taskId) ? <TypingIndicator /> : null}
      {message.side === 'employee' &&
      message.taskStatus === 'failed' &&
      message.taskResult === undefined &&
      message.taskId ? (
        <TaskCrashedPanel
          taskId={message.taskId}
          continuing={continuingTaskIds.has(message.taskId)}
          onContinue={onContinueTask}
          t={t}
        />
      ) : null}
    </div>
  )

  const stamp = message.at.trim().length > 0 ? <div className="chat-message__stamp">{message.at}</div> : null

  return (
    <div className={messageClass(message)}>
      {isMe ? (
        <div className="chat-message__stack chat-message__stack--me">
          <div className="chat-message__head chat-message__head--me">
            <div className="chat-message__sender">
              <div className="chat-message__sender-line1">{userLabel}</div>
              <div className="chat-message__sender-line2">{t('ui.chat.userSenderCaption')}</div>
            </div>
            <ChatMessageAvatar imageUrl={message.senderAvatarUrl} label={userLabel} fallbackLetter={userLabel} altTemplate={avatarAlt} />
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
              <div className="chat-message__sender-line1">{isSystem ? systemLabel : employeeLabel}</div>
              {peerSecondary ? <div className="chat-message__sender-line2">{peerSecondary}</div> : null}
            </div>
          </div>
          {bubble}
          {stamp}
        </div>
      )}
    </div>
  )
})

export const EmployeeChatPanel = React.memo(function EmployeeChatPanel({
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
  const { serverMessages, optimisticUser, streamingAssistant, loading, sending, error, lastResult, refresh, sendMessage, isProcessAlive, hasMoreMessages, loadingMore, loadMoreMessages } =
    useEmployeeChatMessages(apiBase, locale, selectedEmployeeId, chatSenderProfile, onEmployeeTasksRefresh)

  // Track continuing execution state per task
  const [continuingTaskIds, setContinuingTaskIds] = React.useState<Set<string>>(new Set())

  // Refresh messages when parent signals a task completed
  React.useEffect(() => {
    if (chatMessagesRefreshTick) {
      refresh()
    }
  }, [chatMessagesRefreshTick, refresh])

  const historyRef = React.useRef<ChatHistoryListHandle>(null)

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

  const sendFromDraft = React.useCallback(async () => {
    if (!selectedEmployeeId || !selectedEmployee) return
    const text = messageDraft.trim()
    if (!text) return
    onMessageDraftChange('')
    // Sending always returns focus to the latest message, even if the user had
    // scrolled up to read older history.
    requestAnimationFrame(() => historyRef.current?.scrollToBottom('smooth'))
    try {
      await sendMessage(text)
    } catch {
      onMessageDraftChange(text)
    }
  }, [messageDraft, onMessageDraftChange, selectedEmployee, selectedEmployeeId, sendMessage])

  const handleContinueTask = React.useCallback(async (taskId: string) => {
    setContinuingTaskIds(prev => new Set(prev).add(taskId))
    try {
      const url = `${apiBase}/api/tasks/${encodeURIComponent(taskId)}/rerun`
      const res = await fetch(url, {
        method: 'POST',
        headers: {
          'x-lang': locale,
          'Content-Type': 'application/json',
        },
      })
      if (!res.ok) {
        throw new Error(await res.text())
      }
      // Refresh messages to show the restarted task
      await refresh()
    } catch (e) {
      console.error('Failed to continue task:', e)
    } finally {
      setContinuingTaskIds(prev => {
        const next = new Set(prev)
        next.delete(taskId)
        return next
      })
    }
  }, [apiBase, locale, refresh])

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

  const defaultSenderName = t('ui.chat.senderDefaultName')
  const avatarAlt = t('ui.chat.avatarAlt')
  const systemLabel = t('ui.chat.systemSenderLabel')

  const historyHeader = loading || lastResult ? (
    <>
      {loading ? <div className="chat-history__status">{t('ui.chat.loadingHistory')}</div> : null}
      {!loading && lastResult ? <TaskResultPanel result={lastResult} t={t} /> : null}
    </>
  ) : null

  const renderMessage = React.useCallback(
    (message: DisplayMessage) => (
      <ChatMessageItem
        message={message}
        employeeName={selectedEmployee?.name ?? ''}
        employeeDepartment={selectedEmployee?.department ?? ''}
        employeeRole={selectedEmployee?.role ?? ''}
        defaultSenderName={defaultSenderName}
        systemLabel={systemLabel}
        avatarAlt={avatarAlt}
        isProcessAlive={isProcessAlive}
        continuingTaskIds={continuingTaskIds}
        onContinueTask={handleContinueTask}
        t={t}
      />
    ),
    [selectedEmployee?.name, selectedEmployee?.department, selectedEmployee?.role, defaultSenderName, systemLabel, avatarAlt, isProcessAlive, continuingTaskIds, handleContinueTask, t],
  )

  return (
    <div className="chat-layout">
      {selectedEmployee ? (
        <>
          <ChatHistoryList<DisplayMessage>
            ref={historyRef}
            items={displayMessages}
            getKey={(m) => m.id}
            renderItem={renderMessage}
            header={historyHeader}
            scrollDownLabel={t('ui.chat.scrollToLatest')}
            hasMoreAbove={hasMoreMessages}
            loadingMore={loadingMore}
            onLoadMore={loadMoreMessages}
            loadMoreLabel={t('ui.chat.loadMore')}
          />
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
})
