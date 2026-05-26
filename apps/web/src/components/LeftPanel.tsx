import React from 'react'
import type { GitRepo } from '../features/git/gitApi'
import { EmployeeDirectoryRecord, EmployeeList } from './EmployeeList'
import { EmployeeTaskList } from './EmployeeTaskList'
import type { AgentTaskRecord, AgentTaskDetail } from '../features/employee-tasks/employeeTasksApi'
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
  onFireEmployee: (id: string) => void
  deletingEmployeeId: string | null
  activeNav: NavMenu
  creatingEmployee: boolean
  employeeCreateError: string
  workspaceConfigured: boolean
  status: string
  t: (key: string) => string
  onCreateEmployee: () => void
  showArchived: boolean
  onToggleArchived: () => void
  archivedEmployees: EmployeeDirectoryRecord[]
  reinstateEmployeeId: string | null
  onReinstateEmployee: (id: string) => void
  handoverEmployeeId: string | null
  onHandoverEmployee: (id: string) => void
  hardDeletingEmployeeId: string | null
  onHardDeleteEmployee: (id: string) => void
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
  showRequirementsArchived: boolean
  onToggleRequirementsArchived: () => void
  archivedRequirements: RequirementSummary[]
  onAbandonRequirement: (id: string) => void
  onReinstateRequirement: (id: string) => void
  onHardDeleteRequirement: (id: string) => void
  abandoningRequirementId: string | null
  reinstatingRequirementId: string | null
  hardDeletingRequirementId: string | null
  employeeTasks: AgentTaskRecord[]
  employeeTasksLoading: boolean
  employeeTasksError: string | null
  employeeTasksExploring: boolean
  onEmployeeTasksExplore: () => void
  rerunningTaskId: string | null
  onRerunEmployeeTask: (taskId: string) => void
  stoppingTaskId: string | null
  onStopEmployeeTask: (taskId: string) => void
  onFetchEmployeeTaskDetail: (taskId: string) => Promise<AgentTaskDetail>
  locale: string
}

export function LeftPanel({
  panelKey,
  sidePanelWidth,
  employees,
  selectedEmployeeId,
  onSelectEmployee,
  onFireEmployee,
  deletingEmployeeId,
  activeNav,
  creatingEmployee,
  employeeCreateError,
  workspaceConfigured,
  status,
  t,
  onCreateEmployee,
  showArchived,
  onToggleArchived,
  archivedEmployees,
  reinstateEmployeeId,
  onReinstateEmployee,
  handoverEmployeeId,
  onHandoverEmployee,
  hardDeletingEmployeeId,
  onHardDeleteEmployee,
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
  showRequirementsArchived,
  onToggleRequirementsArchived,
  archivedRequirements,
  onAbandonRequirement,
  onReinstateRequirement,
  onHardDeleteRequirement,
  abandoningRequirementId,
  reinstatingRequirementId,
  hardDeletingRequirementId,
  employeeTasks,
  employeeTasksLoading,
  employeeTasksError,
  employeeTasksExploring,
  onEmployeeTasksExplore,
  rerunningTaskId,
  onRerunEmployeeTask,
  stoppingTaskId,
  onStopEmployeeTask,
  onFetchEmployeeTaskDetail,
  locale,
}: LeftPanelProps) {
  const selectedEmployeeName = React.useMemo(() => {
    if (!selectedEmployeeId) return null
    const pool = showArchived ? archivedEmployees : employees
    return pool.find((e) => e.id === selectedEmployeeId)?.name ?? null
  }, [selectedEmployeeId, showArchived, archivedEmployees, employees])

  const renderPanelBody = () => {
    if (activeNav === 'chat') {
      return (
        <>
          <div className="employee-list__toolbar">
            <button
              type="button"
              className="employee-list__hire-btn"
              onClick={() => {
                console.debug('[employee:create] sidebar button clicked')
                onCreateEmployee()
              }}
              disabled={creatingEmployee}
            >
              {creatingEmployee ? t('ui.employeeList.creating') : t('ui.employeeList.create')}
            </button>
            <button
              type="button"
              className="employee-list__toggle-btn"
              onClick={onToggleArchived}
              title={showArchived ? t('ui.employeeList.showActive') : t('ui.employeeList.showArchived')}
            >
              <i className={`iconfont ${showArchived ? 'icon-filmetoChat' : 'icon-filmetoChat'}`}></i>
              <span>{showArchived ? t('ui.employeeList.showActive') : t('ui.employeeList.showArchived')}</span>
            </button>
          </div>
          {employeeCreateError ? (
            <div className="side-panel__error employee-list__toolbar-error">{employeeCreateError}</div>
          ) : null}
          <div className="side-panel__chat-body">
            <div className="side-panel__employees">
              <EmployeeList
                employees={showArchived ? archivedEmployees : employees}
                selectedEmployeeId={selectedEmployeeId}
                onSelectEmployee={onSelectEmployee}
                onFireEmployee={onFireEmployee}
                deletingEmployeeId={deletingEmployeeId}
                t={t}
                isArchivedView={showArchived}
                reinstateEmployeeId={reinstateEmployeeId}
                onReinstateEmployee={onReinstateEmployee}
                onHandoverEmployee={onHandoverEmployee}
                onHardDeleteEmployee={onHardDeleteEmployee}
                handoverEmployeeId={handoverEmployeeId}
                hardDeletingEmployeeId={hardDeletingEmployeeId}
              />
            </div>
            <section className="side-panel__employee-tasks" aria-label={t('ui.employeeTasks.listTitle')}>
              <EmployeeTaskList
                tasks={employeeTasks}
                loading={employeeTasksLoading}
                error={employeeTasksError}
                selectedEmployeeId={selectedEmployeeId}
                selectedEmployeeName={selectedEmployeeName}
                locale={locale}
                exploring={employeeTasksExploring}
                onExplore={onEmployeeTasksExplore}
                rerunningTaskId={rerunningTaskId}
                onRerunTask={onRerunEmployeeTask}
                stoppingTaskId={stoppingTaskId}
                onStopTask={onStopEmployeeTask}
                onFetchTaskDetail={onFetchEmployeeTaskDetail}
                t={t}
              />
            </section>
          </div>
        </>
      )
    }

    if (activeNav === 'requirements') {
      return (
        <>
          <div className="employee-list__toolbar">
            <button
              className="employee-list__toggle-btn"
              onClick={onToggleRequirementsArchived}
              title={showRequirementsArchived ? t('ui.requirements.showActive') : t('ui.requirements.showArchived')}
            >
              <i className={`iconfont ${showRequirementsArchived ? 'icon-filmetoChat' : 'icon-filmetoChat'}`}></i>
              <span>{showRequirementsArchived ? t('ui.requirements.showActive') : t('ui.requirements.showArchived')}</span>
            </button>
          </div>
          <RequirementList
            items={showRequirementsArchived ? archivedRequirements : requirements}
            selectedId={selectedRequirementId}
            onSelect={onSelectRequirement}
            phaseLabel={requirementPhaseLabel}
            t={t}
            isArchivedView={showRequirementsArchived}
            onAbandonRequirement={onAbandonRequirement}
            onReinstateRequirement={onReinstateRequirement}
            onHardDeleteRequirement={onHardDeleteRequirement}
            abandoningId={abandoningRequirementId}
            reinstatingId={reinstatingRequirementId}
            hardDeletingId={hardDeletingRequirementId}
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
