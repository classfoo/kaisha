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

  it('shows failed explore error when no stream events were emitted', () => {
    const display = wireToDisplay({
      id: 'msg_task_process_failed',
      role: 'task_process',
      content: 'no_enabled_coding_tool',
      created_at_ms: 0,
      task_status: 'failed',
      task_id: 'task_failed_1',
      stream_events: [
        {
          type: 'result',
          summary: 'no_enabled_coding_tool',
          is_error: true,
          prompt_tokens: 0,
          completion_tokens: 0,
          total_tokens: 0,
        },
      ],
    })

    expect(display.taskStatus).toBe('failed')
    expect(display.content).toBe('no_enabled_coding_tool')
    expect(display.streaming?.blocks.some((block) => block.type === 'result' && block.isError)).toBe(true)
  })

  it('shows failed explore error when stream events are absent', () => {
    const display = wireToDisplay({
      id: 'msg_task_process_failed_empty',
      role: 'task_process',
      content: 'no_enabled_coding_tool',
      created_at_ms: 0,
      task_status: 'failed',
      task_id: 'task_failed_1',
    })

    expect(display.taskStatus).toBe('failed')
    expect(display.content).toBe('no_enabled_coding_tool')
    expect(display.streaming).toBeUndefined()
  })
})
