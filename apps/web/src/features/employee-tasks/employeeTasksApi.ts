export type AgentTaskStatus =
  | 'pending'
  | 'running'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'queued_rerun'

export type AgentTaskKind =
  | 'employee_hire'
  | 'requirement_agent'
  | 'review_opinion'
  | 'review_revision'
  | 'review_summary'
  | 'review_pipeline'
  | 'autonomy_explore'
  | 'autonomy_execute'
  | 'work_task_execute'

export type AgentTaskRecord = {
  id: string
  kind: AgentTaskKind
  content: string
  workdir: string
  tool_instance_id: string | null
  tool_name: string | null
  tool_kind?: string | null
  executor_id: string | null
  status: AgentTaskStatus
  created_at_ms: number
  started_at_ms: number | null
  ended_at_ms: number | null
  exit_code: number | null
  error: string | null
  output_preview: string | null
  model?: string | null
  prompt_tokens?: number
  completion_tokens?: number
  total_tokens?: number
  parent_task_id?: string | null
  context?: Record<string, unknown>
}

export type AgentTaskExecutionInfo = {
  tool_instance_id: string | null
  tool_name: string | null
  tool_kind: string | null
  model: string | null
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
  exit_code: number | null
  error: string | null
  duration_ms: number | null
}

export type AgentTaskDetail = {
  task: AgentTaskRecord
  output: string | null
  execution: AgentTaskExecutionInfo
}

async function readError(res: Response): Promise<string> {
  const text = await res.text()
  return text || `HTTP ${res.status}`
}

export function createEmployeeTasksApi(apiBase: string, locale: string) {
  const headers = { 'x-lang': locale }

  return {
    async listByExecutor(executorId: string, limit = 50): Promise<AgentTaskRecord[]> {
      const params = new URLSearchParams({
        executor_id: executorId,
        limit: String(limit),
      })
      const res = await fetch(`${apiBase}/api/tasks?${params}`, { headers })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async getDetail(taskId: string): Promise<AgentTaskDetail> {
      const res = await fetch(`${apiBase}/api/tasks/${encodeURIComponent(taskId)}/detail`, { headers })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async triggerExplore(employeeId: string): Promise<void> {
      const res = await fetch(`${apiBase}/api/employees/${encodeURIComponent(employeeId)}/autonomy/explore`, {
        method: 'POST',
        headers,
      })
      if (!res.ok) throw new Error(await readError(res))
    },

    async rerun(taskId: string): Promise<AgentTaskRecord> {
      const res = await fetch(`${apiBase}/api/tasks/${encodeURIComponent(taskId)}/rerun`, {
        method: 'POST',
        headers,
      })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async stop(taskId: string): Promise<AgentTaskRecord> {
      const res = await fetch(`${apiBase}/api/tasks/${encodeURIComponent(taskId)}/stop`, {
        method: 'POST',
        headers,
      })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },
  }
}
