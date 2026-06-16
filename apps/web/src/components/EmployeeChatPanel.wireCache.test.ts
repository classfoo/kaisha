import { describe, expect, it } from 'vitest'
import { wireToDisplay } from './EmployeeChatPanel'
import type { ChatWireMessage } from '../features/employee-chat/useEmployeeChatMessages'

function taskProcess(id: string, texts: string[], status: ChatWireMessage['task_status']): ChatWireMessage {
  return {
    id,
    role: 'task_process',
    content: '',
    created_at_ms: 0,
    task_status: status,
    stream_events: texts.map((text) => ({ type: 'assistant_text', text })),
  }
}

describe('wireToDisplay caching for live task_process messages', () => {
  it('reflects appended stream_events for the same message id (regression: real-time SSE)', () => {
    const id = 'msg_task_process_live'

    const first = wireToDisplay(taskProcess(id, ['hello'], 'running'))
    expect(first.content).toBe('hello')

    // The conversation watch SSE delivers the same message id with more events
    // as the code agent streams. The display must update, not return a stale
    // cached value keyed only on the id.
    const second = wireToDisplay(taskProcess(id, ['hello', ' world'], 'running'))
    expect(second.content).toBe('hello world')

    const third = wireToDisplay(taskProcess(id, ['hello', ' world', '!'], 'running'))
    expect(third.content).toBe('hello world!')
  })

  it('reflects growing assistant text when event count is unchanged (regression: partial text)', () => {
    const id = 'msg_task_process_partial'
    const first = wireToDisplay({
      id,
      role: 'task_process',
      content: '',
      created_at_ms: 0,
      task_status: 'running',
      stream_events: [{ type: 'assistant_text', text: 'hello' }],
    })
    expect(first.content).toBe('hello')

    const second = wireToDisplay({
      id,
      role: 'task_process',
      content: '',
      created_at_ms: 0,
      task_status: 'running',
      stream_events: [{ type: 'assistant_text', text: 'hello world' }],
    })
    expect(second.content).toBe('hello world')
  })

  it('reflects status transition from running to completed for the same id', () => {
    const id = 'msg_task_process_finalize'

    const running = wireToDisplay(taskProcess(id, ['done'], 'running'))
    expect(running.pending).toBe(true)

    const completed = wireToDisplay(taskProcess(id, ['done'], 'completed'))
    expect(completed.pending).toBe(false)
  })
})
