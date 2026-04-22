import React from 'react'
import { EmployeeDirectoryRecord, EmployeeList } from './EmployeeList'
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
                onClick={onCreateEmployee}
                disabled={!workspaceConfigured || creatingEmployee}
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

    if (activeNav === 'build' || activeNav === 'test') {
      const chainItems =
        activeNav === 'build'
          ? ['ui.leftPanel.build.items.resolve', 'ui.leftPanel.build.items.bundle', 'ui.leftPanel.build.items.package']
          : ['ui.leftPanel.test.items.unit', 'ui.leftPanel.test.items.integration', 'ui.leftPanel.test.items.e2e']
      return (
        <>
          <div className="side-panel__section-title">
            {activeNav === 'build' ? t('ui.leftPanel.build.title') : t('ui.leftPanel.test.title')}
          </div>
          <div className="settings-list">
            {chainItems.map((key) => (
              <div key={key} className="settings-list__row">
                <div>{t(key)}</div>
              </div>
            ))}
          </div>
        </>
      )
    }

    return (
      <div className="employee-list__empty">
        {activeNav === 'home' ? t('ui.leftPanel.home.empty') : t('ui.leftPanel.produce.empty')}
      </div>
    )
  }

  return (
    <div className="side-panel-wrap" style={{ width: `${sidePanelWidth}px` }} key={panelKey}>
      <aside className="side-panel" data-tauri-drag-region>
        <div className="side-panel__brand">{t('ui.brand')}</div>
        {renderPanelBody()}
      </aside>
      <div className="side-panel-resizer" onMouseDown={onResizeMouseDown} />
    </div>
  )
}
