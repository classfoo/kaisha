export type AgentTaskStatus =
  | 'pending'
  | 'running'
  | 'completed'
  | 'failed'
  | 'cancelled'

export type AgentTaskKind =
  | 'employee_hire'
  | 'requirement_agent'
  | 'review_opinion'
  | 'review_revision'
  | 'review_summary'
  | 'review_pipeline'

export type AgentTaskRecord = {
  id: string
  kind: AgentTaskKind
  content: string
  workdir: string
  tool_instance_id: string | null
  tool_name: string | null
  executor_id: string | null
  status: AgentTaskStatus
  created_at_ms: number
  started_at_ms: number | null
  ended_at_ms: number | null
  exit_code: number | null
  error: string | null
  output_preview: string | null
  parent_task_id?: string | null
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
  }
}
