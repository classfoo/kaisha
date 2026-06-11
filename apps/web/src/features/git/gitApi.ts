export type GitRepo = {
  id: string
  name: string
  is_main: boolean
  path: string
  created_at_ms: number
  initialized: boolean
}

export type GitRepoStatus = {
  branch: string
  clean: boolean
  ahead: number
  behind: number
  staged: number
  unstaged: number
  untracked: number
  porcelain: string
}

export type GitCommandOutput = {
  stdout: string
  stderr: string
  exit_code: number
}

export type GitBranch = {
  name: string
  current: boolean
  remote: boolean
}

export type GitBranchList = {
  current: string
  branches: GitBranch[]
}

export type GitTreeEntry = {
  name: string
  path: string
  is_dir: boolean
  size: number
}

export type GitTreeListing = {
  path: string
  entries: GitTreeEntry[]
}

export type GitFileContent = {
  path: string
  content: string
  size: number
  binary: boolean
  truncated: boolean
}

export type GitOperation =
  | { operation: 'status' }
  | { operation: 'add'; paths?: string[]; all?: boolean }
  | { operation: 'reset'; mode?: string; paths?: string[] }
  | { operation: 'commit'; message: string; all?: boolean }
  | { operation: 'log'; max_count?: number; oneline?: boolean }
  | { operation: 'branch'; name?: string; delete?: boolean; list?: boolean }
  | { operation: 'checkout'; target: string; create?: boolean }
  | { operation: 'merge'; branch: string }
  | { operation: 'pull'; remote?: string; branch?: string }
  | { operation: 'push'; remote?: string; branch?: string; set_upstream?: boolean }
  | { operation: 'fetch'; remote?: string; prune?: boolean }
  | { operation: 'remote'; name?: string; url?: string; remove?: boolean; list?: boolean }
  | { operation: 'clone'; url: string; directory?: string }
  | { operation: 'diff'; cached?: boolean; paths?: string[] }
  | { operation: 'stash'; action: string; message?: string }
  | { operation: 'tag'; name: string; message?: string; delete?: boolean; list?: boolean }
  | { operation: 'raw'; args: string[] }

async function readError(res: Response): Promise<string> {
  const text = await res.text()
  return text || `HTTP ${res.status}`
}

export function createGitApi(apiBase: string, locale: string) {
  const headers = { 'x-lang': locale }

  return {
    async listRepos(): Promise<{ repos: GitRepo[]; main_repo_id: string }> {
      const res = await fetch(`${apiBase}/api/git/repos`, { headers })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async getRepo(id: string): Promise<{ repo: GitRepo; status: GitRepoStatus | null }> {
      const res = await fetch(`${apiBase}/api/git/repos/${encodeURIComponent(id)}`, { headers })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async createRepo(name: string, id?: string): Promise<{ repo: GitRepo; status: GitRepoStatus | null }> {
      const res = await fetch(`${apiBase}/api/git/repos`, {
        method: 'POST',
        headers: { ...headers, 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, id }),
      })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async runOperation(repoId: string, op: GitOperation): Promise<GitCommandOutput> {
      const res = await fetch(`${apiBase}/api/git/repos/${encodeURIComponent(repoId)}/op`, {
        method: 'POST',
        headers: { ...headers, 'Content-Type': 'application/json' },
        body: JSON.stringify(op),
      })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async listBranches(repoId: string): Promise<GitBranchList> {
      const res = await fetch(`${apiBase}/api/git/repos/${encodeURIComponent(repoId)}/branches`, {
        headers,
      })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async listTree(repoId: string, path: string): Promise<GitTreeListing> {
      const url = `${apiBase}/api/git/repos/${encodeURIComponent(repoId)}/tree?path=${encodeURIComponent(path)}`
      const res = await fetch(url, { headers })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },

    async readFile(repoId: string, path: string): Promise<GitFileContent> {
      const url = `${apiBase}/api/git/repos/${encodeURIComponent(repoId)}/file?path=${encodeURIComponent(path)}`
      const res = await fetch(url, { headers })
      if (!res.ok) throw new Error(await readError(res))
      return res.json()
    },
  }
}
