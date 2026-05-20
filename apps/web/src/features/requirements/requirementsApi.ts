export type RequirementPhase =
  | 'collection'
  | 'review'
  | 'confirm'
  | 'development'
  | 'testing'
  | 'release'

export type RequirementSummary = {
  id: string
  title: string
  phase: RequirementPhase
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

export type ReviewOpinion = {
  employee_id: string
  employee_name: string
  role: string
  role_key: string | null
  content: string
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
