import React from 'react'
import type { useGitWorkspace } from '../features/git/useGitWorkspace'

type GitWorkspace = ReturnType<typeof useGitWorkspace>

type GitPanelProps = {
  git: GitWorkspace
  t: (key: string) => string
}

export function GitPanel({ git, t }: GitPanelProps) {
  const {
    selectedRepo,
    status,
    loading,
    busy,
    error,
    lastOutput,
    runOperation,
  } = git

  const [commitMessage, setCommitMessage] = React.useState('')
  const [branchTarget, setBranchTarget] = React.useState('')
  const [remoteName, setRemoteName] = React.useState('origin')
  const [remoteUrl, setRemoteUrl] = React.useState('')
  const [rawArgs, setRawArgs] = React.useState('status')

  const run = (op: Parameters<typeof runOperation>[0]) => {
    void runOperation(op)
  }

  if (loading) {
    return <div className="git-panel__status">{t('ui.git.loading')}</div>
  }

  if (!selectedRepo) {
    return <div className="git-panel__empty">{t('ui.git.selectRepo')}</div>
  }

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

      {status ? (
        <section className="git-panel__section">
          <h3 className="git-panel__section-title">{t('ui.git.statusTitle')}</h3>
          <div className="git-panel__status-grid">
            <span>{t('ui.git.branch')}</span>
            <span>{status.branch}</span>
            <span>{t('ui.git.worktree')}</span>
            <span>
              {status.clean
                ? t('ui.git.clean')
                : t('ui.git.dirtySummary')
                    .replace('{staged}', String(status.staged))
                    .replace('{unstaged}', String(status.unstaged))
                    .replace('{untracked}', String(status.untracked))}
            </span>
            {(status.ahead > 0 || status.behind > 0) ? (
              <>
                <span>{t('ui.git.sync')}</span>
                <span>
                  {t('ui.git.aheadBehind')
                    .replace('{ahead}', String(status.ahead))
                    .replace('{behind}', String(status.behind))}
                </span>
              </>
            ) : null}
          </div>
          {status.porcelain.trim() ? (
            <pre className="git-panel__porcelain">{status.porcelain}</pre>
          ) : null}
        </section>
      ) : null}

      <section className="git-panel__section">
        <h3 className="git-panel__section-title">{t('ui.git.actionsTitle')}</h3>
        <div className="git-panel__actions">
          <button type="button" className="action-btn" disabled={busy} onClick={() => run({ operation: 'status' })}>
            {t('ui.git.action.status')}
          </button>
          <button type="button" className="action-btn" disabled={busy} onClick={() => run({ operation: 'add', all: true })}>
            {t('ui.git.action.stageAll')}
          </button>
          <button type="button" className="action-btn" disabled={busy} onClick={() => run({ operation: 'fetch', prune: true })}>
            {t('ui.git.action.fetch')}
          </button>
          <button
            type="button"
            className="action-btn"
            disabled={busy}
            onClick={() => run({ operation: 'pull', remote: remoteName || undefined })}
          >
            {t('ui.git.action.pull')}
          </button>
          <button
            type="button"
            className="action-btn"
            disabled={busy}
            onClick={() =>
              run({
                operation: 'push',
                remote: remoteName || undefined,
                set_upstream: true,
              })
            }
          >
            {t('ui.git.action.push')}
          </button>
          <button type="button" className="action-btn" disabled={busy} onClick={() => run({ operation: 'branch', list: true })}>
            {t('ui.git.action.branches')}
          </button>
          <button type="button" className="action-btn" disabled={busy} onClick={() => run({ operation: 'log', max_count: 20 })}>
            {t('ui.git.action.log')}
          </button>
          <button type="button" className="action-btn" disabled={busy} onClick={() => run({ operation: 'stash', action: 'list' })}>
            {t('ui.git.action.stashList')}
          </button>
          <button type="button" className="action-btn" disabled={busy} onClick={() => run({ operation: 'remote', list: true })}>
            {t('ui.git.action.remotes')}
          </button>
        </div>
      </section>

      <section className="git-panel__section">
        <h3 className="git-panel__section-title">{t('ui.git.commitTitle')}</h3>
        <textarea
          className="git-panel__input git-panel__textarea"
          value={commitMessage}
          onChange={(e) => setCommitMessage(e.target.value)}
          placeholder={t('ui.git.commitPlaceholder')}
          rows={3}
        />
        <button
          type="button"
          className="action-btn"
          disabled={busy || !commitMessage.trim()}
          onClick={() => {
            run({ operation: 'commit', message: commitMessage.trim() })
            setCommitMessage('')
          }}
        >
          {t('ui.git.action.commit')}
        </button>
      </section>

      <section className="git-panel__section git-panel__section--row">
        <div className="git-panel__field">
          <label className="git-panel__label">{t('ui.git.branchTitle')}</label>
          <input
            className="git-panel__input"
            value={branchTarget}
            onChange={(e) => setBranchTarget(e.target.value)}
            placeholder={t('ui.git.branchPlaceholder')}
          />
        </div>
        <div className="git-panel__field-actions">
          <button
            type="button"
            className="action-btn"
            disabled={busy || !branchTarget.trim()}
            onClick={() => run({ operation: 'checkout', target: branchTarget.trim() })}
          >
            {t('ui.git.action.checkout')}
          </button>
          <button
            type="button"
            className="action-btn"
            disabled={busy || !branchTarget.trim()}
            onClick={() => run({ operation: 'checkout', target: branchTarget.trim(), create: true })}
          >
            {t('ui.git.action.checkoutNew')}
          </button>
        </div>
      </section>

      <section className="git-panel__section git-panel__section--row">
        <div className="git-panel__field">
          <label className="git-panel__label">{t('ui.git.remoteTitle')}</label>
          <input
            className="git-panel__input"
            value={remoteName}
            onChange={(e) => setRemoteName(e.target.value)}
            placeholder={t('ui.git.remoteNamePlaceholder')}
          />
          <input
            className="git-panel__input"
            value={remoteUrl}
            onChange={(e) => setRemoteUrl(e.target.value)}
            placeholder={t('ui.git.remoteUrlPlaceholder')}
          />
        </div>
        <button
          type="button"
          className="action-btn"
          disabled={busy || !remoteName.trim() || !remoteUrl.trim()}
          onClick={() =>
            run({
              operation: 'remote',
              name: remoteName.trim(),
              url: remoteUrl.trim(),
            })
          }
        >
          {t('ui.git.action.addRemote')}
        </button>
      </section>

      <section className="git-panel__section">
        <h3 className="git-panel__section-title">{t('ui.git.rawTitle')}</h3>
        <input
          className="git-panel__input"
          value={rawArgs}
          onChange={(e) => setRawArgs(e.target.value)}
          placeholder={t('ui.git.rawPlaceholder')}
        />
        <button
          type="button"
          className="action-btn"
          disabled={busy || !rawArgs.trim()}
          onClick={() => {
            const args = rawArgs.trim().split(/\s+/).filter(Boolean)
            run({ operation: 'raw', args })
          }}
        >
          {t('ui.git.action.runRaw')}
        </button>
      </section>

      {outputText ? (
        <section className="git-panel__section">
          <h3 className="git-panel__section-title">{t('ui.git.outputTitle')}</h3>
          <pre className="git-panel__output">{outputText}</pre>
        </section>
      ) : null}
    </div>
  )
}
