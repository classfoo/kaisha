import * as React from 'react'

export type ChatWireMessage = {
  id: string
  role: string
  content: string
  created_at_ms: number
  sender_name?: string | null
  sender_avatar_url?: string | null
  task_id?: string | null
  task_status?: 'running' | 'completed' | 'failed' | null
  stream_events?: StreamEventPayload[] | null
  result_meta?: ChatResultMeta | null
}

export type ChatSenderProfile = {
  name: string
  avatarUrl: string
}

export type ChatResultMeta = {
  exit_code: number
  tool_instance_id: string
  tool_kind: string
  model: string
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
  task_id?: string | null
  output_preview?: string | null
}

export type StreamingToolCall = {
  id: string
  name: string
  inputSummary: string
  outputPreview?: string
  isError?: boolean
  status: 'running' | 'success' | 'error'
}

export type StreamingSessionInfo = {
  model?: string
  sessionId?: string
  tools: string[]
  cwd?: string
}

export type StreamingResultSummary = {
  summary?: string
  model?: string
  promptTokens: number
  completionTokens: number
  totalTokens: number
  isError: boolean
}

/** A single content block within a streaming response. Blocks are appended in
 *  arrival order and group same-type content together, solving the "flat blob"
 *  rendering problem where tool I/O and text were all mashed together. */
export type StreamingBlock =
  | { type: 'text'; text: string }
  | { type: 'thinking'; text: string }
  | { type: 'tool_call'; id: string; name: string; inputSummary: string; outputPreview?: string; isError?: boolean; status: 'running' | 'success' | 'error' }
  | { type: 'session_info'; model?: string; sessionId?: string; tools: string[]; cwd?: string }
  | { type: 'result'; summary?: string; model?: string; promptTokens: number; completionTokens: number; totalTokens: number; isError: boolean }

export type StreamingAssistantState = {
  text: string
  thinking: string
  toolCalls: StreamingToolCall[]
  session: StreamingSessionInfo | null
  result: StreamingResultSummary | null
  /** Ordered blocks for section-based rendering. */
  blocks: StreamingBlock[]
  /** Wall-clock when the first event arrived; useful for showing elapsed time. */
  startedAtMs: number | null
}

const EMPTY_STREAM: StreamingAssistantState = {
  text: '',
  thinking: '',
  toolCalls: [],
  session: null,
  result: null,
  blocks: [],
  startedAtMs: null,
}

type MessagesResponse = { messages: ChatWireMessage[] }
type PostMessageResponse = { messages: ChatWireMessage[]; last_result: ChatResultMeta }

export type StreamEventPayload =
  | { type: 'start'; model?: string; session_id?: string; tools?: string[]; cwd?: string }
  | { type: 'assistant_text'; text: string }
  | { type: 'thinking'; text: string }
  | { type: 'tool_use'; id: string; name: string; input_summary: string }
  | {
      type: 'tool_result'
      tool_use_id: string
      output_preview: string
      is_error: boolean
    }
  | {
      type: 'result'
      summary?: string
      model?: string
      prompt_tokens?: number
      completion_tokens?: number
      total_tokens?: number
      is_error?: boolean
    }
  | { type: 'raw'; text: string }

function parseSseBuffer(buffer: string): { rest: string; events: { event: string; data: string }[] } {
  const parts = buffer.split('\n\n')
  const rest = parts.pop() ?? ''
  const events: { event: string; data: string }[] = []
  for (const block of parts) {
    if (!block.trim()) continue
    let ev = 'message'
    const dataLines: string[] = []
    for (const rawLine of block.split('\n')) {
      const line = rawLine.replace(/\r$/, '')
      if (line.startsWith('event:')) ev = line.slice(6).trimStart()
      else if (line.startsWith('data:')) dataLines.push(line.slice(5).replace(/^ /, ''))
    }
    const data = dataLines.join('\n')
    if (data.length > 0) events.push({ event: ev, data })
  }
  return { rest, events }
}

function nowMs(prev: number | null): number {
  return prev ?? Date.now()
}

/** Helper: find index of a block by tool_use id within the blocks array. */
function findToolBlockIndex(blocks: StreamingBlock[], id: string): number {
  return blocks.findIndex(
    (b) => b.type === 'tool_call' && b.id === id,
  )
}

export function applyStreamingEvent(
  prev: StreamingAssistantState,
  event: { event: string; data: string },
): StreamingAssistantState {
  const startedAtMs = prev.startedAtMs ?? Date.now()
  if (event.event === 'delta') {
    try {
      const j = JSON.parse(event.data) as { text?: string }
      if (j.text) {
        const blocks = [...prev.blocks]
        const last = blocks.length > 0 ? blocks[blocks.length - 1] : null
        if (last && last.type === 'text') {
          blocks[blocks.length - 1] = { ...last, text: last.text + j.text }
        } else {
          blocks.push({ type: 'text', text: j.text })
        }
        return { ...prev, text: prev.text + j.text, blocks, startedAtMs: nowMs(prev.startedAtMs) }
      }
    } catch {
      /* ignore malformed delta */
    }
    return prev
  }

  let payload: StreamEventPayload | null = null
  try {
    payload = JSON.parse(event.data) as StreamEventPayload
  } catch {
    return prev
  }
  if (!payload || typeof payload !== 'object' || !('type' in payload)) {
    return prev
  }

  switch (payload.type) {
    case 'start': {
      const sessionBlock: StreamingBlock = {
        type: 'session_info',
        model: payload.model,
        sessionId: payload.session_id,
        tools: payload.tools ?? [],
        cwd: payload.cwd,
      }
      return {
        ...prev,
        session: {
          model: payload.model,
          sessionId: payload.session_id,
          tools: payload.tools ?? [],
          cwd: payload.cwd,
        },
        blocks: [...prev.blocks, sessionBlock],
        startedAtMs,
      }
    }
    case 'assistant_text': {
      const blocks = [...prev.blocks]
      const last = blocks.length > 0 ? blocks[blocks.length - 1] : null
      if (last && last.type === 'text') {
        blocks[blocks.length - 1] = { ...last, text: last.text + payload.text }
      } else {
        blocks.push({ type: 'text', text: payload.text })
      }
      return { ...prev, text: prev.text + payload.text, blocks, startedAtMs }
    }
    case 'thinking': {
      const blocks = [...prev.blocks]
      const last = blocks.length > 0 ? blocks[blocks.length - 1] : null
      if (last && last.type === 'thinking') {
        blocks[blocks.length - 1] = { ...last, text: last.text + payload.text }
      } else {
        blocks.push({ type: 'thinking', text: payload.text })
      }
      return { ...prev, thinking: prev.thinking + payload.text, blocks, startedAtMs }
    }
    case 'tool_use': {
      const useId = payload.id
      const existing = prev.toolCalls.find((tc) => tc.id === useId)
      const next: StreamingToolCall = {
        id: useId,
        name: payload.name,
        inputSummary: payload.input_summary,
        status: existing?.status ?? 'running',
        outputPreview: existing?.outputPreview,
        isError: existing?.isError,
      }
      const toolCalls = existing
        ? prev.toolCalls.map((tc) => (tc.id === useId ? next : tc))
        : [...prev.toolCalls, next]

      // Block-level: create a new tool_call block (or update existing by id)
      const blocks = [...prev.blocks]
      const blockIdx = findToolBlockIndex(blocks, useId)
      const blockTool: StreamingBlock = {
        type: 'tool_call',
        id: useId,
        name: payload.name,
        inputSummary: payload.input_summary,
        status: existing?.status ?? 'running',
        outputPreview: existing?.outputPreview,
        isError: existing?.isError,
      }
      if (blockIdx >= 0) {
        blocks[blockIdx] = blockTool
      } else {
        blocks.push(blockTool)
      }
      return { ...prev, toolCalls, blocks, startedAtMs }
    }
    case 'tool_result': {
      const toolCalls = prev.toolCalls.map((tc) => {
        if (tc.id !== payload.tool_use_id) return tc
        return {
          ...tc,
          outputPreview: payload.output_preview,
          isError: payload.is_error,
          status: payload.is_error ? 'error' : 'success',
        } satisfies StreamingToolCall
      })
      // If we received a tool_result before the matching tool_use (rare), add a stub.
      const hasMatch = prev.toolCalls.some((tc) => tc.id === payload.tool_use_id)
      const merged = hasMatch
        ? toolCalls
        : [
            ...toolCalls,
            {
              id: payload.tool_use_id,
              name: '',
              inputSummary: '',
              outputPreview: payload.output_preview,
              isError: payload.is_error,
              status: payload.is_error ? ('error' as const) : ('success' as const),
            },
          ]

      // Block-level: find the matching tool_call block and update it
      const blocks = [...prev.blocks]
      const blockIdx = findToolBlockIndex(blocks, payload.tool_use_id)
      if (blockIdx >= 0) {
        const existingBlock = blocks[blockIdx]
        if (existingBlock.type === 'tool_call') {
          blocks[blockIdx] = {
            ...existingBlock,
            outputPreview: payload.output_preview,
            isError: payload.is_error,
            status: payload.is_error ? 'error' : 'success',
          }
        }
      } else if (!hasMatch) {
        // Stub block for orphan tool_result
        blocks.push({
          type: 'tool_call',
          id: payload.tool_use_id,
          name: '',
          inputSummary: '',
          outputPreview: payload.output_preview,
          isError: payload.is_error,
          status: payload.is_error ? 'error' : 'success',
        })
      }
      return { ...prev, toolCalls: merged, blocks, startedAtMs }
    }
    case 'result': {
      const resultBlock: StreamingBlock = {
        type: 'result',
        summary: payload.summary,
        model: payload.model,
        promptTokens: payload.prompt_tokens ?? 0,
        completionTokens: payload.completion_tokens ?? 0,
        totalTokens: payload.total_tokens ?? 0,
        isError: Boolean(payload.is_error),
      }
      return {
        ...prev,
        result: {
          summary: payload.summary,
          model: payload.model,
          promptTokens: payload.prompt_tokens ?? 0,
          completionTokens: payload.completion_tokens ?? 0,
          totalTokens: payload.total_tokens ?? 0,
          isError: Boolean(payload.is_error),
        },
        blocks: [...prev.blocks, resultBlock],
        startedAtMs,
      }
    }
    case 'raw': {
      const blocks = [...prev.blocks]
      const last = blocks.length > 0 ? blocks[blocks.length - 1] : null
      if (last && last.type === 'text') {
        blocks[blocks.length - 1] = { ...last, text: last.text + payload.text }
      } else {
        blocks.push({ type: 'text', text: payload.text })
      }
      return { ...prev, text: prev.text + payload.text, blocks, startedAtMs }
    }
    default:
      return prev
  }
}

/** Reconstruct a StreamingAssistantState from a sequence of saved stream events.
 *  Used when restoring streaming state from a persisted task_process message. */
export function reconstructStreamingState(streamEvents: StreamEventPayload[]): StreamingAssistantState {
  let state: StreamingAssistantState = { ...EMPTY_STREAM }
  for (const event of streamEvents) {
    // Map the saved event into the { event, data } format that applyStreamingEvent expects.
    // The saved events are already typed as StreamEventPayload, so we reconstruct the SSE format.
    const sseEvent: { event: string; data: string } = {
      event: 'message',
      data: JSON.stringify(event),
    }

    // For the 'start' event type, applyStreamingEvent expects event==='start'
    // but our SSE parser maps named SSE events. We need to use the event type directly.
    switch (event.type) {
      case 'start':
        sseEvent.event = 'start'
        break
      case 'assistant_text':
        sseEvent.event = 'delta'
        break
      case 'thinking':
        sseEvent.event = 'thinking'
        break
      case 'tool_use':
        sseEvent.event = 'tool_use'
        break
      case 'tool_result':
        sseEvent.event = 'tool_result'
        break
      case 'result':
        sseEvent.event = 'result'
        break
      case 'raw':
        sseEvent.event = 'delta'
        break
    }

    state = applyStreamingEvent(state, sseEvent)
  }
  return state
}

/** Upserts incoming messages into the existing list by id, preserving order and
 *  appending any messages not seen before. Used to apply incremental updates
 *  pushed from the conversation-watch SSE stream. */
export function mergeMessages(
  prev: ChatWireMessage[],
  incoming: ChatWireMessage[],
): ChatWireMessage[] {
  if (incoming.length === 0) return prev
  const incomingById = new Map(incoming.map((m) => [m.id, m]))
  const result = prev.map((m) => incomingById.get(m.id) ?? m)
  const existingIds = new Set(prev.map((m) => m.id))
  for (const m of incoming) {
    if (!existingIds.has(m.id)) result.push(m)
  }
  return result
}

export function useEmployeeChatMessages(
  apiBase: string,
  locale: string,
  employeeId: string | null,
  senderProfile: ChatSenderProfile,
  onStreamDone?: () => void,
) {
  const [serverMessages, setServerMessages] = React.useState<ChatWireMessage[]>([])
  const serverMessagesRef = React.useRef(serverMessages)
  serverMessagesRef.current = serverMessages
  const [optimisticUser, setOptimisticUser] = React.useState<ChatWireMessage | null>(null)
  const [streamingAssistant, setStreamingAssistant] =
    React.useState<StreamingAssistantState>(EMPTY_STREAM)
  const [loading, setLoading] = React.useState(false)
  const [sending, setSending] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  const [lastResult, setLastResult] = React.useState<ChatResultMeta | null>(null)

  // Mirror `sending` into a ref so the conversation-watch subscription can avoid
  // fighting the active POST stream (which renders its own live overlay).
  const sendingRef = React.useRef(false)
  React.useEffect(() => {
    sendingRef.current = sending
  }, [sending])

  const headers = React.useMemo(
    () => ({
      'x-lang': locale,
    }),
    [locale],
  )

  const refresh = React.useCallback(async () => {
    if (!employeeId) {
      setServerMessages([])
      setLastResult(null)
      setError(null)
      setOptimisticUser(null)
      setStreamingAssistant(EMPTY_STREAM)
      return
    }
    setOptimisticUser(null)
    setStreamingAssistant(EMPTY_STREAM)
    setLastResult(null)
    setLoading(true)
    setError(null)
    try {
      const url = `${apiBase}/api/employees/${encodeURIComponent(employeeId)}/messages`
      const res = await fetch(url, { headers })
      const text = await res.text()
      if (!res.ok) throw new Error(text || `HTTP ${res.status}`)
      const data = JSON.parse(text) as MessagesResponse
      setServerMessages(data.messages ?? [])
    } catch (e) {
      setServerMessages([])
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [apiBase, employeeId, headers])

  React.useEffect(() => {
    void refresh()
  }, [refresh])

  // Subscribe to the conversation watch stream so output written to the
  // conversation file (by interactive chat, autonomy execute/explore, or any
  // background task run) appears in the message list in real time. Updates are
  // skipped while a POST stream is active to avoid double-rendering the reply.
  React.useEffect(() => {
    if (!employeeId) return
    const url = `${apiBase}/api/employees/${encodeURIComponent(employeeId)}/conversation/stream`
    const source = new EventSource(url)
    const onUpdate = (event: MessageEvent) => {
      if (sendingRef.current) return
      try {
        const data = JSON.parse(event.data) as { messages?: ChatWireMessage[] }
        if (data.messages && data.messages.length > 0) {
          setServerMessages((prev) => mergeMessages(prev, data.messages ?? []))
        }
      } catch {
        /* ignore malformed update */
      }
    }
    source.addEventListener('update', onUpdate as EventListener)
    return () => {
      source.removeEventListener('update', onUpdate as EventListener)
      source.close()
    }
  }, [apiBase, employeeId])

  // Process alive polling: check if running tasks are still alive
  const [processAliveStatus, setProcessAliveStatus] = React.useState<Map<string, boolean>>(new Map())

  React.useEffect(() => {
    const pollInterval = 2000 // 2 seconds
    const poll = async () => {
      // Read latest messages from ref to avoid re-running effect on every SSE update
      const runningTaskIds = serverMessagesRef.current
        .filter(m => m.role === 'task_process' && m.task_status === 'running' && m.task_id)
        .map(m => m.task_id as string)

      if (runningTaskIds.length === 0) {
        setProcessAliveStatus(new Map())
        return
      }

      const newStatus = new Map<string, boolean>()
      for (const taskId of runningTaskIds) {
        try {
          const url = `${apiBase}/api/tasks/${encodeURIComponent(taskId)}/alive`
          const res = await fetch(url, { headers })
          if (res.ok) {
            const data = await res.json() as { alive: boolean }
            newStatus.set(taskId, data.alive)
          } else {
            newStatus.set(taskId, false)
          }
        } catch {
          newStatus.set(taskId, false)
        }
      }
      setProcessAliveStatus(newStatus)
    }

    // Initial poll
    void poll()
    const timer = setInterval(poll, pollInterval)
    return () => clearInterval(timer)
  }, [apiBase, headers])

  const isProcessAlive = React.useCallback((taskId: string): boolean => {
    return processAliveStatus.get(taskId) ?? false
  }, [processAliveStatus])

  const sendMessage = React.useCallback(
    async (body: string) => {
      const trimmed = body.trim()
      if (!employeeId || !trimmed) return
      const optimistic: ChatWireMessage = {
        id: `optimistic-user-${Date.now()}`,
        role: 'user',
        content: trimmed,
        created_at_ms: Date.now(),
        sender_name: senderProfile.name,
        ...(senderProfile.avatarUrl ? { sender_avatar_url: senderProfile.avatarUrl } : {}),
      }
      setOptimisticUser(optimistic)
      setStreamingAssistant(EMPTY_STREAM)
      setSending(true)
      setError(null)
      try {
        const url = `${apiBase}/api/employees/${encodeURIComponent(employeeId)}/messages/stream`
        const res = await fetch(url, {
          method: 'POST',
          headers: {
            ...headers,
            'Content-Type': 'application/json',
            Accept: 'text/event-stream',
          },
          body: JSON.stringify({
            content: trimmed,
            sender_name: senderProfile.name,
            ...(senderProfile.avatarUrl ? { sender_avatar_url: senderProfile.avatarUrl } : {}),
          }),
        })
        if (!res.ok) {
          const t = await res.text()
          throw new Error(t || `HTTP ${res.status}`)
        }
        const reader = res.body?.getReader()
        if (!reader) throw new Error('No response body')
        const dec = new TextDecoder()
        let buf = ''
        const applyEvents = (events: { event: string; data: string }[]) => {
          for (const ev of events) {
            if (ev.event === 'done') {
              const data = JSON.parse(ev.data) as PostMessageResponse
              setServerMessages(data.messages ?? [])
              setLastResult(data.last_result ?? null)
              setStreamingAssistant(EMPTY_STREAM)
              setOptimisticUser(null)
              onStreamDone?.()
            } else if (ev.event === 'error') {
              let msg = ev.data
              try {
                const j = JSON.parse(ev.data) as { message?: string }
                if (j.message) msg = j.message
              } catch {
                /* use raw */
              }
              throw new Error(msg)
            } else {
              setStreamingAssistant((prev) => applyStreamingEvent(prev, ev))
            }
          }
        }

        while (true) {
          const { done, value } = await reader.read()
          if (done) break
          buf += dec.decode(value, { stream: true })
          const parsed = parseSseBuffer(buf)
          buf = parsed.rest
          applyEvents(parsed.events)
        }
        if (buf.trim()) {
          const parsed = parseSseBuffer(`${buf}\n\n`)
          applyEvents(parsed.events)
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e))
        throw e
      } finally {
        setSending(false)
        setOptimisticUser(null)
        setStreamingAssistant(EMPTY_STREAM)
      }
    },
    [apiBase, employeeId, headers, onStreamDone, senderProfile.name, senderProfile.avatarUrl],
  )

  return {
    serverMessages,
    optimisticUser,
    streamingAssistant,
    loading,
    sending,
    error,
    lastResult,
    refresh,
    sendMessage,
    isProcessAlive,
  }
}
