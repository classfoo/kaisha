import React from 'react'
import type { useGitWorkspace } from '../features/git/useGitWorkspace'
import type { GitFileContent } from '../features/git/gitApi'
import { GitFileTree } from './GitFileTree'
import { GitFileDialog } from './GitFileDialog'

type GitWorkspace = ReturnType<typeof useGitWorkspace>

type GitPanelProps = {
  git: GitWorkspace
  t: (key: string) => string
}

const REMOTE_PREFIX = 'remote:'

export function GitPanel({ git, t }: GitPanelProps) {
  const {
    selectedRepo,
    status,
    branches,
    currentBranch,
    loading,
    busy,
    error,
    lastOutput,
    runOperation,
    checkoutBranch,
    listTree,
    readFile,
  } = git

  const [treeReloadKey, setTreeReloadKey] = React.useState(0)
  const [commitOpen, setCommitOpen] = React.useState(false)
  const [commitMessage, setCommitMessage] = React.useState('')
  const [branchOpen, setBranchOpen] = React.useState(false)
  const [newBranch, setNewBranch] = React.useState('')
  const [outputOpen, setOutputOpen] = React.useState(false)

  const [fileOpen, setFileOpen] = React.useState(false)
  const [fileTitle, setFileTitle] = React.useState('')
  const [fileLoading, setFileLoading] = React.useState(false)
  const [fileError, setFileError] = React.useState<string | null>(null)
  const [fileContent, setFileContent] = React.useState<GitFileContent | null>(null)

  const reloadTree = React.useCallback(() => setTreeReloadKey((k) => k + 1), [])

  const run = React.useCallback(
    async (op: Parameters<typeof runOperation>[0]) => {
      const out = await runOperation(op)
      reloadTree()
      return out
    },
    [runOperation, reloadTree],
  )

  const handleOpenFile = React.useCallback(
    async (path: string, name: string) => {
      setFileOpen(true)
      setFileTitle(path || name)
      setFileLoading(true)
      setFileError(null)
      setFileContent(null)
      try {
        const file = await readFile(path)
        setFileContent(file)
      } catch (e) {
        setFileError(e instanceof Error ? e.message : String(e))
      } finally {
        setFileLoading(false)
      }
    },
    [readFile],
  )

  const handleBranchChange = React.useCallback(
    async (value: string) => {
      if (!value) return
      const target = value.startsWith(REMOTE_PREFIX)
        ? value.slice(REMOTE_PREFIX.length).split('/').slice(1).join('/')
        : value
      if (!target || target === currentBranch) return
      await checkoutBranch(target)
      reloadTree()
    },
    [checkoutBranch, currentBranch, reloadTree],
  )

  if (loading) {
    return <div className="git-panel__status">{t('ui.git.loading')}</div>
  }

  if (!selectedRepo) {
    return <div className="git-panel__empty">{t('ui.git.selectRepo')}</div>
  }

  const localBranches = branches.filter((b) => !b.remote)
  const localNames = new Set(localBranches.map((b) => b.name))
  const remoteBranches = branches.filter(
    (b) => b.remote && !localNames.has(b.name.split('/').slice(1).join('/')),
  )

  const selectValue = localNames.has(currentBranch) ? currentBranch : ''

  const statusSummary = status
    ? status.clean
      ? t('ui.git.clean')
      : t('ui.git.dirtySummary')
          .replace('{staged}', String(status.staged))
          .replace('{unstaged}', String(status.unstaged))
          .replace('{untracked}', String(status.untracked))
    : ''

  const syncSummary =
    status && (status.ahead > 0 || status.behind > 0)
      ? t('ui.git.aheadBehind')
          .replace('{ahead}', String(status.ahead))
          .replace('{behind}', String(status.behind))
      : ''

  const outputText = lastOutput
    ? [lastOutput.stdout, lastOutput.stderr].filter((s) => s.trim().length > 0).join('\n')
    : ''

  return (
    <div className="git-panel">
      <header className="git-panel__header">
        <h2 className="git-panel__title">{selectedRepo.name}</h2>
        {selectedRepo.is_main ? (
          <span className="git-repo-list__badge">{t('ui.git.mainBadge')}</span>
        ) : null}
        <span className="git-panel__path">{selectedRepo.path}</span>
      </header>

      {error ? <div className="git-panel__error">{error}</div> : null}

      <div className="git-panel__bar">
        <label className="git-panel__branch">
          <span className="git-panel__branch-icon" aria-hidden="true">⎇</span>
          <select
            className="git-panel__branch-select"
            value={selectValue}
            disabled={busy || branches.length === 0}
            onChange={(e) => void handleBranchChange(e.target.value)}
            aria-label={t('ui.git.branchSwitch')}
          >
            {selectValue === '' ? (
              <option value="">{currentBranch || t('ui.git.detached')}</option>
            ) : null}
            {localBranches.length > 0 ? (
              <optgroup label={t('ui.git.localBranches')}>
                {localBranches.map((b) => (
                  <option key={b.name} value={b.name}>
                    {b.name}
                  </option>
                ))}
              </optgroup>
            ) : null}
            {remoteBranches.length > 0 ? (
              <optgroup label={t('ui.git.remoteBranches')}>
                {remoteBranches.map((b) => (
                  <option key={b.name} value={`${REMOTE_PREFIX}${b.name}`}>
                    {b.name}
                  </option>
                ))}
              </optgroup>
            ) : null}
          </select>
        </label>
        {statusSummary ? <span className="git-panel__bar-status">{statusSummary}</span> : null}
        {syncSummary ? <span className="git-panel__bar-sync">{syncSummary}</span> : null}
      </div>

      <div className="git-panel__toolbar">
        <button type="button" className="action-btn" disabled={busy} onClick={() => void run({ operation: 'status' })}>
          {t('ui.git.action.refresh')}
        </button>
        <button type="button" className="action-btn" disabled={busy} onClick={() => void run({ operation: 'add', all: true })}>
          {t('ui.git.action.stageAll')}
        </button>
        <button
          type="button"
          className={`action-btn${commitOpen ? ' action-btn--active' : ''}`}
          disabled={busy}
          onClick={() => setCommitOpen((v) => !v)}
        >
          {t('ui.git.action.commit')}
        </button>
        <button type="button" className="action-btn" disabled={busy} onClick={() => void run({ operation: 'fetch', prune: true })}>
          {t('ui.git.action.fetch')}
        </button>
        <button type="button" className="action-btn" disabled={busy} onClick={() => void run({ operation: 'pull' })}>
          {t('ui.git.action.pull')}
        </button>
        <button
          type="button"
          className="action-btn"
          disabled={busy}
          onClick={() => void run({ operation: 'push', set_upstream: true })}
        >
          {t('ui.git.action.push')}
        </button>
        <button
          type="button"
          className={`action-btn${branchOpen ? ' action-btn--active' : ''}`}
          disabled={busy}
          onClick={() => setBranchOpen((v) => !v)}
        >
          {t('ui.git.action.checkoutNew')}
        </button>
        <button
          type="button"
          className="action-btn"
          disabled={busy}
          onClick={() => {
            void run({ operation: 'log', max_count: 30 })
            setOutputOpen(true)
          }}
        >
          {t('ui.git.action.log')}
        </button>
      </div>

      {commitOpen ? (
        <div className="git-panel__inline">
          <textarea
            className="git-panel__input git-panel__textarea"
            value={commitMessage}
            onChange={(e) => setCommitMessage(e.target.value)}
            placeholder={t('ui.git.commitPlaceholder')}
            rows={2}
          />
          <button
            type="button"
            className="action-btn"
            disabled={busy || !commitMessage.trim()}
            onClick={async () => {
              await run({ operation: 'commit', message: commitMessage.trim() })
              setCommitMessage('')
              setCommitOpen(false)
            }}
          >
            {t('ui.git.action.commit')}
          </button>
        </div>
      ) : null}

      {branchOpen ? (
        <div className="git-panel__inline">
          <input
            className="git-panel__input"
            value={newBranch}
            onChange={(e) => setNewBranch(e.target.value)}
            placeholder={t('ui.git.branchPlaceholder')}
          />
          <button
            type="button"
            className="action-btn"
            disabled={busy || !newBranch.trim()}
            onClick={async () => {
              await checkoutBranch(newBranch.trim(), true)
              reloadTree()
              setNewBranch('')
              setBranchOpen(false)
            }}
          >
            {t('ui.git.action.checkoutNew')}
          </button>
        </div>
      ) : null}

      <section className="git-panel__section git-panel__section--grow">
        <div className="git-panel__section-head">
          <h3 className="git-panel__section-title">{t('ui.git.filesTitle')}</h3>
          <button type="button" className="git-panel__link" disabled={busy} onClick={reloadTree}>
            {t('ui.git.tree.refresh')}
          </button>
        </div>
        <p className="git-panel__hint">{t('ui.git.tree.hint')}</p>
        <div className="git-panel__tree">
          <GitFileTree listTree={listTree} onOpenFile={handleOpenFile} reloadKey={treeReloadKey} t={t} />
        </div>
      </section>

      {outputText ? (
        <section className="git-panel__section">
          <div className="git-panel__section-head">
            <h3 className="git-panel__section-title">{t('ui.git.outputTitle')}</h3>
            <button type="button" className="git-panel__link" onClick={() => setOutputOpen((v) => !v)}>
              {outputOpen ? t('ui.git.hide') : t('ui.git.show')}
            </button>
          </div>
          {outputOpen ? <pre className="git-panel__output">{outputText}</pre> : null}
        </section>
      ) : null}

      <GitFileDialog
        open={fileOpen}
        loading={fileLoading}
        error={fileError}
        title={fileTitle}
        file={fileContent}
        t={t}
        onClose={() => setFileOpen(false)}
      />
    </div>
  )
}
