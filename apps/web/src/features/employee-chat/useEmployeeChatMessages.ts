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

type MessagesResponse = { messages: ChatWireMessage[] }
type PostMessageResponse = { messages: ChatWireMessage[]; last_result: ChatResultMeta }

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

export function useEmployeeChatMessages(
  apiBase: string,
  locale: string,
  employeeId: string | null,
  senderProfile: ChatSenderProfile,
  onStreamDone?: () => void,
) {
  const [serverMessages, setServerMessages] = React.useState<ChatWireMessage[]>([])
  const [optimisticUser, setOptimisticUser] = React.useState<ChatWireMessage | null>(null)
  const [streamingAssistantText, setStreamingAssistantText] = React.useState('')
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
      setStreamingAssistantText('')
      return
    }
    setOptimisticUser(null)
    setStreamingAssistantText('')
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
      setStreamingAssistantText('')
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
            if (ev.event === 'delta') {
              try {
                const j = JSON.parse(ev.data) as { text?: string }
                if (j.text) setStreamingAssistantText((s) => s + j.text)
              } catch {
                /* ignore malformed chunk */
              }
            } else if (ev.event === 'done') {
              const data = JSON.parse(ev.data) as PostMessageResponse
              setServerMessages(data.messages ?? [])
              setLastResult(data.last_result ?? null)
              setStreamingAssistantText('')
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
        setStreamingAssistantText('')
      }
    },
    [apiBase, employeeId, headers, onStreamDone, senderProfile.name, senderProfile.avatarUrl],
  )

  return {
    serverMessages,
    optimisticUser,
    streamingAssistantText,
    loading,
    sending,
    error,
    lastResult,
    refresh,
    sendMessage,
  }
}
