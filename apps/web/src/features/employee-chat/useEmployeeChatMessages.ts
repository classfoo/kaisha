import * as React from 'react'

export type ChatWireMessage = {
  id: string
  role: string
  content: string
  created_at_ms: number
  sender_name?: string | null
  sender_avatar_url?: string | null
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

export type StreamingAssistantState = {
  text: string
  thinking: string
  toolCalls: StreamingToolCall[]
  session: StreamingSessionInfo | null
  result: StreamingResultSummary | null
  /** Wall-clock when the first event arrived; useful for showing elapsed time. */
  startedAtMs: number | null
}

const EMPTY_STREAM: StreamingAssistantState = {
  text: '',
  thinking: '',
  toolCalls: [],
  session: null,
  result: null,
  startedAtMs: null,
}

type MessagesResponse = { messages: ChatWireMessage[] }
type PostMessageResponse = { messages: ChatWireMessage[]; last_result: ChatResultMeta }

type StreamEventPayload =
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

export function applyStreamingEvent(
  prev: StreamingAssistantState,
  event: { event: string; data: string },
): StreamingAssistantState {
  const startedAtMs = prev.startedAtMs ?? Date.now()
  if (event.event === 'delta') {
    try {
      const j = JSON.parse(event.data) as { text?: string }
      if (j.text) {
        return { ...prev, text: prev.text + j.text, startedAtMs: nowMs(prev.startedAtMs) }
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
    case 'start':
      return {
        ...prev,
        session: {
          model: payload.model,
          sessionId: payload.session_id,
          tools: payload.tools ?? [],
          cwd: payload.cwd,
        },
        startedAtMs,
      }
    case 'assistant_text':
      return { ...prev, text: prev.text + payload.text, startedAtMs }
    case 'thinking':
      return { ...prev, thinking: prev.thinking + payload.text, startedAtMs }
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
      return { ...prev, toolCalls, startedAtMs }
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
      return { ...prev, toolCalls: merged, startedAtMs }
    }
    case 'result':
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
        startedAtMs,
      }
    case 'raw':
      return { ...prev, text: prev.text + payload.text, startedAtMs }
    default:
      return prev
  }
}

export function useEmployeeChatMessages(
  apiBase: string,
  locale: string,
  employeeId: string | null,
  senderProfile: ChatSenderProfile,
  onStreamDone?: () => void,
) {
  const [serverMessages, setServerMessages] = React.useState<ChatWireMessage[]>([])
  const [optimisticUser, setOptimisticUser] = React.useState<ChatWireMessage | null>(null)
  const [streamingAssistant, setStreamingAssistant] =
    React.useState<StreamingAssistantState>(EMPTY_STREAM)
  const [loading, setLoading] = React.useState(false)
  const [sending, setSending] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  const [lastResult, setLastResult] = React.useState<ChatResultMeta | null>(null)

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
  }
}
