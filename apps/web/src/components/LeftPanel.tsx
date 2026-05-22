import React from 'react'
import type { GitRepo } from '../features/git/gitApi'
import { EmployeeDirectoryRecord, EmployeeList } from './EmployeeList'
import { GitRepoList } from './GitRepoList'
import type { RequirementPhase, RequirementSummary } from '../features/requirements/requirementsApi'
import { RequirementList } from './RequirementList'
import { NavMenu } from './LeftSidebar'

type LeftPanelProps = {
  panelKey: string
  sidePanelWidth: number
  employees: EmployeeDirectoryRecord[]
  selectedEmployeeId: string | null
  onSelectEmployee: (id: string) => void
  activeNav: NavMenu
  creatingEmployee: boolean
  employeeCreateError: string
  workspaceConfigured: boolean
  status: string
  t: (key: string) => string
  onCreateEmployee: () => void
  onResizeMouseDown: (event: React.MouseEvent<HTMLDivElement>) => void
  gitRepos: GitRepo[]
  selectedGitRepoId: string | null
  onSelectGitRepo: (id: string) => void
  newGitRepoName: string
  onNewGitRepoNameChange: (value: string) => void
  onCreateGitRepo: () => void
  gitBusy: boolean
  gitError: string | null
  gitLoading: boolean
  requirements: RequirementSummary[]
  selectedRequirementId: string | null
  onSelectRequirement: (id: string) => void
  newRequirementTitle: string
  onNewRequirementTitleChange: (value: string) => void
  onCreateRequirement: () => void
  requirementsBusy: boolean
  requirementsError: string | null
  requirementsLoading: boolean
  requirementPhaseLabel: (phase: RequirementPhase) => string
}

export function LeftPanel({
  panelKey,
  sidePanelWidth,
  employees,
  selectedEmployeeId,
  onSelectEmployee,
  activeNav,
  creatingEmployee,
  employeeCreateError,
  workspaceConfigured,
  status,
  t,
  onCreateEmployee,
  onResizeMouseDown,
  gitRepos,
  selectedGitRepoId,
  onSelectGitRepo,
  newGitRepoName,
  onNewGitRepoNameChange,
  onCreateGitRepo,
  gitBusy,
  gitError,
  gitLoading,
  requirements,
  selectedRequirementId,
  onSelectRequirement,
  newRequirementTitle,
  onNewRequirementTitleChange,
  onCreateRequirement,
  requirementsBusy,
  requirementsError,
  requirementsLoading,
  requirementPhaseLabel,
}: LeftPanelProps) {
  const renderPanelBody = () => {
    if (activeNav === 'chat') {
      return (
        <>
          <EmployeeList
            employees={employees}
            selectedEmployeeId={selectedEmployeeId}
            onSelectEmployee={onSelectEmployee}
            t={t}
          />
          <div className="side-panel__footer">
            <div className="side-panel__toolbar">
              <button
                className="action-btn side-panel__add-employee"
                onClick={() => {
                  console.debug('[employee:create] sidebar button clicked')
                  onCreateEmployee()
                }}
                disabled={creatingEmployee}
              >
                {creatingEmployee ? t('ui.employeeList.creating') : t('ui.employeeList.create')}
              </button>
              {employeeCreateError ? (
                <div className="side-panel__error">{employeeCreateError}</div>
              ) : null}
            </div>
            <div className="side-panel__status">
              <span>{t('ui.backend')}</span>
              <span className={`status status--${status}`}>{status}</span>
            </div>
          </div>
        </>
      )
    }

    if (activeNav === 'requirements') {
      return (
        <>
          <RequirementList
            items={requirements}
            selectedId={selectedRequirementId}
            onSelect={onSelectRequirement}
            phaseLabel={requirementPhaseLabel}
            t={t}
          />
          <div className="side-panel__footer">
            <div className="side-panel__toolbar">
              <input
                className="settings-input side-panel__git-name"
                value={newRequirementTitle}
                onChange={(event) => onNewRequirementTitleChange(event.target.value)}
                placeholder={t('ui.requirements.newTitlePlaceholder')}
                disabled={requirementsBusy || requirementsLoading}
              />
              <button
                type="button"
                className="action-btn side-panel__add-employee"
                onClick={onCreateRequirement}
                disabled={requirementsBusy || requirementsLoading || !newRequirementTitle.trim()}
              >
                {requirementsBusy ? t('ui.requirements.creating') : t('ui.requirements.create')}
              </button>
              {requirementsError ? <div className="side-panel__error">{requirementsError}</div> : null}
            </div>
          </div>
        </>
      )
    }

    if (activeNav === 'git') {
      return (
        <>
          <GitRepoList
            repos={gitRepos}
            selectedRepoId={selectedGitRepoId}
            onSelectRepo={onSelectGitRepo}
            t={t}
          />
          <div className="side-panel__footer">
            <div className="side-panel__toolbar">
              <input
                className="settings-input side-panel__git-name"
                value={newGitRepoName}
                onChange={(event) => onNewGitRepoNameChange(event.target.value)}
                placeholder={t('ui.git.newRepoPlaceholder')}
                disabled={gitBusy || gitLoading}
              />
              <button
                type="button"
                className="action-btn side-panel__add-employee"
                onClick={onCreateGitRepo}
                disabled={gitBusy || gitLoading || !newGitRepoName.trim()}
              >
                {gitBusy ? t('ui.git.creating') : t('ui.git.createRepo')}
              </button>
              {gitError ? <div className="side-panel__error">{gitError}</div> : null}
            </div>
          </div>
        </>
      )
    }

    return (
      <div className="employee-list__empty">
        {t('ui.leftPanel.home.empty')}
      </div>
    )
  }

  return (
    <div className="side-panel-wrap" style={{ width: `${sidePanelWidth}px` }} key={panelKey}>
      <aside className="side-panel" data-tauri-drag-region>
        {renderPanelBody()}
      </aside>
      <div className="side-panel-resizer" onMouseDown={onResizeMouseDown} />
    </div>
  )
}
