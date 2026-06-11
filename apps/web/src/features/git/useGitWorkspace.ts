import * as React from 'react'
import {
  createGitApi,
  type GitBranch,
  type GitCommandOutput,
  type GitFileContent,
  type GitOperation,
  type GitRepo,
  type GitRepoStatus,
  type GitTreeListing,
} from './gitApi'

export function useGitWorkspace(
  apiBase: string,
  locale: string,
  workspaceConfigured: boolean,
  refreshTick: number,
) {
  const api = React.useMemo(() => createGitApi(apiBase, locale), [apiBase, locale])
  const [repos, setRepos] = React.useState<GitRepo[]>([])
  const [mainRepoId, setMainRepoId] = React.useState('main')
  const [selectedRepoId, setSelectedRepoId] = React.useState<string | null>(null)
  const [status, setStatus] = React.useState<GitRepoStatus | null>(null)
  const [branches, setBranches] = React.useState<GitBranch[]>([])
  const [currentBranch, setCurrentBranch] = React.useState('')
  const [loading, setLoading] = React.useState(false)
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  const [lastOutput, setLastOutput] = React.useState<GitCommandOutput | null>(null)

  const selectedRepoIdRef = React.useRef(selectedRepoId)
  selectedRepoIdRef.current = selectedRepoId

  const loadBranches = React.useCallback(
    async (repoId: string) => {
      try {
        const list = await api.listBranches(repoId)
        setBranches(list.branches)
        setCurrentBranch(list.current)
      } catch {
        setBranches([])
        setCurrentBranch('')
      }
    },
    [api],
  )

  const loadRepoDetail = React.useCallback(
    async (repoId: string) => {
      const detail = await api.getRepo(repoId)
      setStatus(detail.status)
      if (detail.repo.initialized) {
        await loadBranches(repoId)
      } else {
        setBranches([])
        setCurrentBranch('')
      }
      return detail
    },
    [api, loadBranches],
  )

  const refresh = React.useCallback(async () => {
    if (!workspaceConfigured) {
      setRepos([])
      setSelectedRepoId(null)
      setStatus(null)
      setBranches([])
      setCurrentBranch('')
      setError(null)
      return
    }
    setLoading(true)
    setError(null)
    try {
      const data = await api.listRepos()
      setRepos(data.repos)
      setMainRepoId(data.main_repo_id)
      const pick =
        data.repos.find((r) => r.id === selectedRepoIdRef.current) ??
        data.repos.find((r) => r.is_main) ??
        data.repos[0] ??
        null
      if (pick) {
        setSelectedRepoId(pick.id)
        await loadRepoDetail(pick.id)
      } else {
        setSelectedRepoId(null)
        setStatus(null)
        setBranches([])
        setCurrentBranch('')
      }
    } catch (e) {
      setRepos([])
      setSelectedRepoId(null)
      setStatus(null)
      setBranches([])
      setCurrentBranch('')
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [api, loadRepoDetail, workspaceConfigured])

  React.useEffect(() => {
    void refresh()
  }, [refresh, refreshTick])

  const selectRepo = React.useCallback(
    async (repoId: string) => {
      setSelectedRepoId(repoId)
      setError(null)
      setLastOutput(null)
      try {
        await loadRepoDetail(repoId)
      } catch (e) {
        setStatus(null)
        setError(e instanceof Error ? e.message : String(e))
      }
    },
    [loadRepoDetail],
  )

  const createRepo = React.useCallback(
    async (name: string, id?: string) => {
      setBusy(true)
      setError(null)
      try {
        const detail = await api.createRepo(name, id)
        const data = await api.listRepos()
        setRepos(data.repos)
        setSelectedRepoId(detail.repo.id)
        setStatus(detail.status)
        return detail.repo
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setBusy(false)
      }
    },
    [api],
  )

  const runOperation = React.useCallback(
    async (op: GitOperation) => {
      if (!selectedRepoId) return null
      setBusy(true)
      setError(null)
      try {
        const out = await api.runOperation(selectedRepoId, op)
        setLastOutput(out)
        if (op.operation === 'status' || op.operation === 'commit' || op.operation === 'add') {
          await loadRepoDetail(selectedRepoId)
        } else if (out.exit_code === 0) {
          await loadRepoDetail(selectedRepoId)
        }
        return out
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setBusy(false)
      }
    },
    [api, loadRepoDetail, selectedRepoId],
  )

  const checkoutBranch = React.useCallback(
    async (branch: string, create = false) => {
      return runOperation({ operation: 'checkout', target: branch, create })
    },
    [runOperation],
  )

  const listTree = React.useCallback(
    async (path: string): Promise<GitTreeListing | null> => {
      if (!selectedRepoId) return null
      return api.listTree(selectedRepoId, path)
    },
    [api, selectedRepoId],
  )

  const readFile = React.useCallback(
    async (path: string): Promise<GitFileContent | null> => {
      if (!selectedRepoId) return null
      return api.readFile(selectedRepoId, path)
    },
    [api, selectedRepoId],
  )

  const selectedRepo = repos.find((r) => r.id === selectedRepoId) ?? null

  return {
    repos,
    mainRepoId,
    selectedRepo,
    selectedRepoId,
    status,
    branches,
    currentBranch,
    loading,
    busy,
    error,
    lastOutput,
    refresh,
    selectRepo,
    createRepo,
    runOperation,
    checkoutBranch,
    listTree,
    readFile,
  }
}
