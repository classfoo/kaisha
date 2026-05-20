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
