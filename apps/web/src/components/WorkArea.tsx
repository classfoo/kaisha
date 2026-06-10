import React from 'react'
import type { ChatSenderProfile } from '../features/employee-chat/useEmployeeChatMessages'
import { EmployeeChatPanel } from './EmployeeChatPanel'
import { EmployeeDirectoryRecord } from './EmployeeList'
import { GitPanel } from './GitPanel'
import { RequirementDetailPanel } from './RequirementDetailPanel'
import type { useGitWorkspace } from '../features/git/useGitWorkspace'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'
import type { RequirementPhase } from '../features/requirements/requirementsApi'
import { RpgHomeEntry } from '../features/rpg-home'
import { NavMenu } from './LeftSidebar'

type WorkAreaProps = {
  workAreaKey: string
  activeNav: NavMenu
  splitPane: React.ReactNode | null
  workspaceConfigured: boolean
  workspacePath: string | null
  workspaceInput: string
  workspaceError: string
  savingWorkspace: boolean
  messageDraft: string
  settingsOpen: boolean
  settingsSection: 'tools' | 'departments' | 'roles' | 'employees' | 'work_rules' | 'language'
  employees: EmployeeDirectoryRecord[]
  selectedEmployeeId: string | null
  apiBase: string
  locale: string
  chatSenderProfile: ChatSenderProfile
  git: ReturnType<typeof useGitWorkspace>
  requirements: ReturnType<typeof useRequirementsWorkspace>
  requirementPhaseLabel: (phase: RequirementPhase) => string
  onEmployeeTasksRefresh?: () => void
  /** When this counter changes, the chat panel will reload messages. */
  chatMessagesRefreshTick?: number
  t: (key: string) => string
  onOpenSettings: () => void
  onSetWorkspaceInput: (value: string) => void
  onSaveWorkspace: () => void
  onMessageDraftChange: (value: string) => void
  onCloseSettings: () => void
  onSetSettingsSection: (value: 'tools' | 'departments' | 'roles' | 'employees' | 'work_rules' | 'language') => void
  renderSettingsCards: () => React.ReactNode
}

export function WorkArea({
  workAreaKey,
  activeNav,
  splitPane,
  workspaceConfigured,
  workspacePath,
  workspaceInput,
  workspaceError,
  savingWorkspace,
  messageDraft,
  settingsOpen,
  settingsSection,
  employees,
  selectedEmployeeId,
  apiBase,
  locale,
  chatSenderProfile,
  git,
  requirements,
  requirementPhaseLabel,
  onEmployeeTasksRefresh,
  chatMessagesRefreshTick,
  t,
  onOpenSettings,
  onSetWorkspaceInput,
  onSaveWorkspace,
  onMessageDraftChange,
  onCloseSettings,
  onSetSettingsSection,
  renderSettingsCards,
}: WorkAreaProps) {
  const needsWorkspaceSetup = workspaceConfigured === false
  const usesSplitLayout = splitPane !== null
  const showChatPanel = activeNav === 'chat'
  const showGitPanel = activeNav === 'git'
  const showRequirementsPanel = activeNav === 'requirements'
  const [searchQuery, setSearchQuery] = React.useState('')
  const searchPlaceholder = workspacePath?.trim()
    ? workspacePath.trim()
    : t('ui.workspace.notConfigured')
  const flushWorkAreaContent = showChatPanel && workspaceConfigured
  const workAreaContentClassName = ['work-area__content', usesSplitLayout ? 'work-area__content--split' : '', flushWorkAreaContent ? 'work-area__content--flush' : '']
    .filter(Boolean)
    .join(' ')

  const actionKeys: string[] = (() => {
    if (activeNav === 'home') return ['ui.actions.settings']
    if (activeNav === 'git' || activeNav === 'requirements') {
      return ['ui.actions.share', 'ui.actions.settings']
    }
    return ['ui.actions.run', 'ui.actions.share', 'ui.actions.settings']
  })()
  const iconForAction = (key: string) => {
    if (key === 'ui.actions.run') {
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true" className="toolbar-icon">
          <path d="M8 6l10 6-10 6V6z" fill="none" stroke="currentColor" strokeWidth="1.8" />
        </svg>
      )
    }
    if (key === 'ui.actions.share') {
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true" className="toolbar-icon">
          <path d="M15 8l-6 4 6 4M6 12h11" fill="none" stroke="currentColor" strokeWidth="1.8" />
        </svg>
      )
    }
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true" className="toolbar-icon">
        <path
          d="M12 8.8a3.2 3.2 0 1 0 0 6.4 3.2 3.2 0 0 0 0-6.4zm8 3.2l-1.8-.7a6.8 6.8 0 0 0-.4-1l.8-1.8-2.1-2.1-1.8.8a6.8 6.8 0 0 0-1-.4L13 4h-2l-.7 1.8a6.8 6.8 0 0 0-1 .4l-1.8-.8-2.1 2.1.8 1.8a6.8 6.8 0 0 0-.4 1L4 12v2l1.8.7a6.8 6.8 0 0 0 .4 1l-.8 1.8 2.1 2.1 1.8-.8a6.8 6.8 0 0 0 1 .4L11 20h2l.7-1.8a6.8 6.8 0 0 0 1-.4l1.8.8 2.1-2.1-.8-1.8a6.8 6.8 0 0 0 .4-1L20 14v-2z"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.6"
        />
      </svg>
    )
  }

  const renderWorkspaceContent = () => {
    if (needsWorkspaceSetup) {
      return (
        <div className="workspace-setup">
          <h2 className="workspace-setup__title">{t('ui.workspace.configureTitle')}</h2>
          <p className="workspace-setup__hint">
            {t('ui.workspace.configureHint')}
          </p>
          <label className="workspace-setup__label" htmlFor="workspace-path">
            {t('ui.workspace.pathLabel')}
          </label>
          <input
            id="workspace-path"
            className="workspace-setup__input"
            value={workspaceInput}
            onChange={(event) => onSetWorkspaceInput(event.target.value)}
            placeholder={t('ui.workspace.placeholder')}
          />
          <button
            className="action-btn workspace-setup__save"
            onClick={onSaveWorkspace}
            disabled={savingWorkspace}
          >
            {savingWorkspace ? t('ui.actions.saving') : t('ui.actions.saveWorkspace')}
          </button>
          {workspaceError ? (
            <p className="workspace-setup__error">{workspaceError}</p>
          ) : null}
        </div>
      )
    }

    if (showChatPanel) {
      return (
        <EmployeeChatPanel
          apiBase={apiBase}
          locale={locale}
          employees={employees}
          selectedEmployeeId={selectedEmployeeId}
          messageDraft={messageDraft}
          onMessageDraftChange={onMessageDraftChange}
          chatSenderProfile={chatSenderProfile}
          onEmployeeTasksRefresh={onEmployeeTasksRefresh}
          chatMessagesRefreshTick={chatMessagesRefreshTick}
          t={t}
        />
      )
    }

    if (showGitPanel) {
      return <GitPanel git={git} t={t} />
    }

    if (showRequirementsPanel) {
      return (
        <RequirementDetailPanel
          requirements={requirements}
          phaseLabel={requirementPhaseLabel}
          t={t}
        />
      )
    }

    if (activeNav === 'home') {
      return <RpgHomeEntry employees={employees} t={t} />
    }

    return (
      <div className="content-placeholder">
        <div>{t('ui.workspace.contentPlaceholder')}</div>
      </div>
    )
  }

  return (
    <>
      <section className="work-area" key={workAreaKey}>
        {usesSplitLayout ? (
          <div className="work-area__split">
            {splitPane}
            <section className="workspace-pane">
              <header className="work-area__topbar" data-tauri-drag-region>
                <div className="topbar__drag" data-tauri-drag-region />
                <div className="topbar__center" onMouseDown={(e) => e.stopPropagation()}>
                  <input
                    className="topbar__search"
                    value={searchQuery}
                    onChange={(event) => setSearchQuery(event.target.value)}
                    placeholder={searchPlaceholder}
                    title={workspacePath?.trim() ? workspacePath.trim() : undefined}
                  />
                </div>
                <div className="topbar__actions" onMouseDown={(e) => e.stopPropagation()}>
                  {actionKeys.map((key) => (
                    <button
                      key={key}
                      className="action-btn action-btn--icon"
                      onClick={key === 'ui.actions.settings' ? onOpenSettings : undefined}
                      aria-label={t(key)}
                      title={t(key)}
                    >
                      {iconForAction(key)}
                    </button>
                  ))}
                </div>
              </header>
              <main className={workAreaContentClassName}>
                {renderWorkspaceContent()}
              </main>
            </section>
          </div>
        ) : (
          <section className="workspace-pane">
            <header className="work-area__topbar" data-tauri-drag-region>
              <div className="topbar__drag" data-tauri-drag-region />
              <div className="topbar__center" onMouseDown={(e) => e.stopPropagation()}>
                <input
                  className="topbar__search"
                  value={searchQuery}
                  onChange={(event) => setSearchQuery(event.target.value)}
                  placeholder={searchPlaceholder}
                  title={workspacePath?.trim() ? workspacePath.trim() : undefined}
                />
              </div>
              <div className="topbar__actions" onMouseDown={(e) => e.stopPropagation()}>
                {actionKeys.map((key) => (
                  <button
                    key={key}
                    className="action-btn action-btn--icon"
                    onClick={key === 'ui.actions.settings' ? onOpenSettings : undefined}
                    aria-label={t(key)}
                    title={t(key)}
                  >
                    {iconForAction(key)}
                  </button>
                ))}
              </div>
            </header>
            <main className={workAreaContentClassName}>
              {renderWorkspaceContent()}
            </main>
          </section>
        )}
      </section>
      {settingsOpen ? (
        <div className="settings-modal">
          <div className="settings-panel">
            <aside className="settings-sidebar">
              <div className="settings-sidebar__title">{t('ui.settings.title')}</div>
              <button
                className={`settings-nav ${settingsSection === 'tools' ? 'settings-nav--active' : ''}`}
                onClick={() => onSetSettingsSection('tools')}
              >
                {t('ui.settings.menus.tools')}
              </button>
              <button
                className={`settings-nav ${settingsSection === 'departments' ? 'settings-nav--active' : ''}`}
                onClick={() => onSetSettingsSection('departments')}
              >
                {t('ui.settings.menus.departments')}
              </button>
              <button
                className={`settings-nav ${settingsSection === 'roles' ? 'settings-nav--active' : ''}`}
                onClick={() => onSetSettingsSection('roles')}
              >
                {t('ui.settings.menus.roles')}
              </button>
              <button
                className={`settings-nav ${settingsSection === 'employees' ? 'settings-nav--active' : ''}`}
                onClick={() => onSetSettingsSection('employees')}
              >
                {t('ui.settings.menus.employees')}
              </button>
              <button
                className={`settings-nav ${settingsSection === 'language' ? 'settings-nav--active' : ''}`}
                onClick={() => onSetSettingsSection('language')}
              >
                {t('ui.settings.menus.language')}
              </button>
              <button className="action-btn settings-close" onClick={onCloseSettings}>
                {t('ui.actions.done')}
              </button>
            </aside>
            <section className="settings-content">
              {renderSettingsCards()}
            </section>
          </div>
        </div>
      ) : null}
    </>
  )
}
