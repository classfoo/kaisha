import React from 'react'
import type { GitRepo } from '../features/git/gitApi'

type GitRepoListProps = {
  repos: GitRepo[]
  selectedRepoId: string | null
  onSelectRepo: (id: string) => void
  t: (key: string) => string
}

export const GitRepoList = React.memo(function GitRepoList({ repos, selectedRepoId, onSelectRepo, t }: GitRepoListProps) {
  return (
    <div className="git-repo-list" role="listbox" aria-label={t('ui.git.repoList')}>
      {repos.length === 0 ? (
        <div className="employee-list__empty">{t('ui.git.repoEmpty')}</div>
      ) : (
        repos.map((repo) => {
          const isActive = repo.id === selectedRepoId
          return (
            <button
              key={repo.id}
              type="button"
              className={`employee-item ${isActive ? 'employee-item--active' : ''}`}
              onClick={() => onSelectRepo(repo.id)}
            >
              <div className="employee-item__avatar" aria-hidden="true">
                {repo.is_main ? '★' : 'G'}
              </div>
              <div className="employee-item__main">
                <div className="employee-item__name">{repo.name}</div>
                <div className="employee-item__snippet">{repo.id}</div>
              </div>
              {repo.is_main ? (
                <span className="git-repo-list__badge">{t('ui.git.mainBadge')}</span>
              ) : null}
            </button>
          )
        })
      )}
    </div>
  )
})
