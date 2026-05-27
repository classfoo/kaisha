export type RequirementPhase =
  | 'collection'
  | 'review'
  | 'confirm'
  | 'development'
  | 'testing'
  | 'release'

export type RequirementConfirmStatus = 'pending' | 'confirmed' | 'abandoned'

export type RequirementSummary = {
  id: string
  title: string
  phase: RequirementPhase
  confirm_status?: RequirementConfirmStatus
  created_at_ms: number
  updated_at_ms: number
  dir_path: string
}

export type RequirementDetail = RequirementSummary & {
  content: string
  subdirs: string[]
}

export type ReviewStatus = 'in_progress' | 'completed'
export type ReviewConclusion = 'adopt' | 'supplement'
export type OpinionItemStatus = 'pending' | 'in_progress' | 'revising' | 'completed' | 'abandoned'
export type OpinionUserAction = 'rerun' | 'pass' | 'fail' | 'abandon'

export type ReviewOpinion = {
  task_id: string
  biz_type: string
  biz_id: string
  employee_id: string
  employee_name: string
  role: string
  role_key: string | null
  status: OpinionItemStatus
  passed: boolean | null
  content: string | null
}

export type RequirementReview = {
  requirement_id: string
  status: ReviewStatus
  started_at_ms: number
  completed_at_ms: number | null
  conclusion: ReviewConclusion | null
  participants: string[]
  opinions: ReviewOpinion[]
  summary: string | null
  passed_count: number
  failed_count: number
  pending_count: number
  undecided_count: number
  abandoned_count: number
  overall_passed: boolean
}

export type DevTaskStatus =
  | 'branch_created'
  | 'in_development'
  | 'dev_complete'
  | 'in_review'
  | 'review_complete'
  | 'merged'

export type DevTask = {
  id: string
  title: string
  assignee?: string
  branch: string
  status: DevTaskStatus
  progress: number
  created_at_ms: number
  updated_at_ms: number
  biz_type: string
  biz_id: string
}

export type RequirementDevelopment = {
  requirement_id: string
  feature_branch: string
  feature_branch_created: boolean
  tasks: DevTask[]
  current_task_id?: string
}

export type WorkRoleDefinition = {
  display_name: string
  aliases: string[]
  duties: Record<string, string>
}

export type WorkRules = {
  version: number
  roles: Record<string, WorkRoleDefinition>
}

async function readError(res: Response): Promise<string> {
  const text = await res.text()
  return text || `HTTP ${res.status}`
}

export function createRequirementsApi(apiBase: string, locale: string) {
  const headers = { 'x-lang': locale }

  return {
    async list(): Promise<RequirementSummary[]> {
      const res = await fetch(`${apiBase}/api/requirements`, { headers })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async get(id: string): Promise<RequirementDetail> {
      const res = await fetch(`${apiBase}/api/requirements/${encodeURIComponent(id)}`, { headers })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async create(payload: {
      title: string
      phase?: RequirementPhase
      content?: string
      id?: string
    }): Promise<RequirementDetail> {
      const res = await fetch(`${apiBase}/api/requirements`, {
        method: 'POST',
        headers: { ...headers, 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async update(
      id: string,
      payload: { title?: string; phase?: RequirementPhase; content?: string },
    ): Promise<RequirementDetail> {
      const res = await fetch(`${apiBase}/api/requirements/${encodeURIComponent(id)}`, {
        method: 'PUT',
        headers: { ...headers, 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async getReview(id: string): Promise<RequirementReview | null> {
      const res = await fetch(`${apiBase}/api/requirements/${encodeURIComponent(id)}/review`, { headers })
      if (res.status === 404) return null
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async runReview(id: string): Promise<RequirementReview> {
      const res = await fetch(`${apiBase}/api/requirements/${encodeURIComponent(id)}/review/run`, {
        method: 'POST',
        headers,
      })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async opinionAction(
      requirementId: string,
      employeeId: string,
      action: OpinionUserAction,
    ): Promise<RequirementReview> {
      const res = await fetch(
        `${apiBase}/api/requirements/${encodeURIComponent(requirementId)}/review/opinions/${encodeURIComponent(employeeId)}/${action}`,
        { method: 'POST', headers },
      )
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async forcePassReview(id: string): Promise<RequirementReview> {
      const res = await fetch(
        `${apiBase}/api/requirements/${encodeURIComponent(id)}/review/force-pass`,
        { method: 'POST', headers },
      )
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async confirm(id: string): Promise<RequirementDetail> {
      const res = await fetch(
        `${apiBase}/api/requirements/${encodeURIComponent(id)}/confirm`,
        { method: 'POST', headers },
      )
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async abandon(id: string): Promise<RequirementDetail> {
      const res = await fetch(
        `${apiBase}/api/requirements/${encodeURIComponent(id)}/abandon`,
        { method: 'POST', headers },
      )
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async reconfirm(id: string): Promise<RequirementDetail> {
      const res = await fetch(
        `${apiBase}/api/requirements/${encodeURIComponent(id)}/reconfirm`,
        { method: 'POST', headers },
      )
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async listArchived(): Promise<RequirementSummary[]> {
      const res = await fetch(`${apiBase}/api/requirements/archived`, { headers })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async reinstate(id: string): Promise<RequirementDetail> {
      const res = await fetch(
        `${apiBase}/api/requirements/${encodeURIComponent(id)}/reinstate`,
        { method: 'POST', headers },
      )
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async hardDelete(id: string): Promise<void> {
      const res = await fetch(
        `${apiBase}/api/requirements/${encodeURIComponent(id)}/hard-delete`,
        { method: 'POST', headers },
      )
      if (!res.ok) throw new Error(await readError(res))
    },

    async getDevelopment(id: string): Promise<RequirementDevelopment | null> {
      const res = await fetch(`${apiBase}/api/requirements/${encodeURIComponent(id)}/development`, { headers })
      if (res.status === 404) return null
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async startDevelopment(id: string): Promise<RequirementDevelopment> {
      const res = await fetch(
        `${apiBase}/api/requirements/${encodeURIComponent(id)}/development`,
        { method: 'POST', headers: { ...headers, 'Content-Type': 'application/json' }, body: JSON.stringify({}) },
      )
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async createDevTask(id: string, payload: { title: string; assignee?: string }): Promise<RequirementDevelopment> {
      const res = await fetch(
        `${apiBase}/api/requirements/${encodeURIComponent(id)}/development/tasks`,
        { method: 'POST', headers: { ...headers, 'Content-Type': 'application/json' }, body: JSON.stringify(payload) },
      )
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async updateDevTask(
      id: string,
      taskId: string,
      payload: { title?: string; assignee?: string; progress?: number },
    ): Promise<RequirementDevelopment> {
      const res = await fetch(
        `${apiBase}/api/requirements/${encodeURIComponent(id)}/development/tasks/${encodeURIComponent(taskId)}`,
        { method: 'PUT', headers: { ...headers, 'Content-Type': 'application/json' }, body: JSON.stringify(payload) },
      )
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async deleteDevTask(id: string, taskId: string): Promise<RequirementDevelopment> {
      const res = await fetch(
        `${apiBase}/api/requirements/${encodeURIComponent(id)}/development/tasks/${encodeURIComponent(taskId)}`,
        { method: 'DELETE', headers },
      )
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async devTaskAction(id: string, taskId: string, action: string): Promise<RequirementDevelopment> {
      const res = await fetch(
        `${apiBase}/api/requirements/${encodeURIComponent(id)}/development/tasks/${encodeURIComponent(taskId)}/${encodeURIComponent(action)}`,
        { method: 'POST', headers: { ...headers, 'Content-Type': 'application/json' }, body: JSON.stringify({ action }) },
      )
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async getWorkRules(): Promise<WorkRules> {
      const res = await fetch(`${apiBase}/api/work-rules`, { headers })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async saveWorkRules(rules: WorkRules): Promise<WorkRules> {
      const res = await fetch(`${apiBase}/api/work-rules`, {
        method: 'PUT',
        headers: { ...headers, 'Content-Type': 'application/json' },
        body: JSON.stringify(rules),
      })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },
  }
}

export const REQUIREMENT_PHASES: RequirementPhase[] = [
  'collection',
  'review',
  'confirm',
  'development',
  'testing',
  'release',
]
