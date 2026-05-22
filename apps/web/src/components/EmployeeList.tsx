import React from 'react'
import { ConfirmDialog } from './ConfirmDialog'

export type EmployeeDirectoryRecord = {
  id: string
  name: string
  department: string
  role: string
  memory_file: string
}

type EmployeeListProps = {
  employees: EmployeeDirectoryRecord[]
  selectedEmployeeId: string | null
  onSelectEmployee: (id: string) => void
  onFireEmployee: (id: string) => void
  deletingEmployeeId: string | null
  t: (key: string) => string
  isArchivedView?: boolean
  reinstateEmployeeId: string | null
  onReinstateEmployee: (id: string) => void
  onHandoverEmployee?: (id: string) => void
  onHardDeleteEmployee?: (id: string) => void
  handoverEmployeeId: string | null
  hardDeletingEmployeeId: string | null
}

export function EmployeeList({
  employees,
  selectedEmployeeId,
  onSelectEmployee,
  onFireEmployee,
  deletingEmployeeId,
  t,
  isArchivedView,
  reinstateEmployeeId,
  onReinstateEmployee,
  onHandoverEmployee,
  onHardDeleteEmployee,
  handoverEmployeeId,
  hardDeletingEmployeeId,
}: EmployeeListProps) {
  const [openMenuId, setOpenMenuId] = React.useState<string | null>(null)
  const [fireConfirmId, setFireConfirmId] = React.useState<string | null>(null)
  const [handoverConfirmId, setHandoverConfirmId] = React.useState<string | null>(null)
  const [hardDeleteConfirmId, setHardDeleteConfirmId] = React.useState<string | null>(null)

  const fireConfirmName = fireConfirmId
    ? employees.find((e) => e.id === fireConfirmId)?.name ?? ''
    : ''
  const handoverConfirmName = handoverConfirmId
    ? employees.find((e) => e.id === handoverConfirmId)?.name ?? ''
    : ''
  const hardDeleteConfirmName = hardDeleteConfirmId
    ? employees.find((e) => e.id === hardDeleteConfirmId)?.name ?? ''
    : ''

  return (
    <div className="employee-list" role="listbox" aria-label={t('ui.employeeList.title')}>
      {employees.length === 0 ? (
        <div className="employee-list__empty">
          {isArchivedView
            ? t('ui.employeeList.archivedEmpty')
            : t('ui.employeeList.empty')}
        </div>
      ) : (
        employees.map((item, index) => {
          const isActive = item.id === selectedEmployeeId
          const unread = index === 0
          const snippet = t('ui.employeeList.snippet').replace('{name}', item.name)
          const recentTime = unread ? '09:24' : t('ui.employeeList.yesterday')
          const isFiring = deletingEmployeeId === item.id
          const isReinstating = reinstateEmployeeId === item.id
          const isHandover = handoverEmployeeId === item.id
          const isHardDeleting = hardDeletingEmployeeId === item.id
          const isBusy = isFiring || isReinstating || isHandover || isHardDeleting
          const menuOpen = openMenuId === item.id
          return (
            <div
              key={item.id}
              className={`employee-item ${isActive ? 'employee-item--active' : ''} ${isArchivedView ? 'employee-item--archived' : ''} ${menuOpen ? 'employee-item--menu-open' : ''}`}
            >
              <button
                type="button"
                className="employee-item__body"
                onClick={() => onSelectEmployee(item.id)}
                disabled={isBusy}
              >
                <div className="employee-item__avatar">{item.name.slice(0, 1).toUpperCase()}</div>
                <div className="employee-item__main">
                  <div className="employee-item__name">{item.name}</div>
                  <div className="employee-item__snippet">
                    {isArchivedView
                      ? `${item.department} / ${item.role}`
                      : snippet}
                  </div>
                </div>
                <div className="employee-item__meta">
                  <span className="employee-item__time">{recentTime}</span>
                  <span
                    className={`employee-item__dot ${unread ? 'employee-item__dot--unread' : ''}`}
                    aria-label={unread ? t('ui.employeeList.unread') : t('ui.employeeList.read')}
                  />
                </div>
              </button>
              <div className="employee-item__menu">
                <button
                  type="button"
                  className="employee-item__menu-btn"
                  title="Menu"
                  onClick={(e) => {
                    e.stopPropagation()
                    setOpenMenuId(menuOpen ? null : item.id)
                  }}
                  disabled={isBusy}
                >
                  <i className="iconfont icon-filmetomore"></i>
                </button>
                {menuOpen && (
                  <div className="employee-item__menu-dropdown">
                    {!isArchivedView && (
                      <button
                        type="button"
                        className="employee-item__menu-dropdown-item employee-item__menu-dropdown-item--fire"
                        onClick={(e) => {
                          e.stopPropagation()
                          setOpenMenuId(null)
                          setFireConfirmId(item.id)
                        }}
                        disabled={isFiring}
                      >
                        {t('ui.employeeList.fire')}
                      </button>
                    )}
                    {isArchivedView && (
                      <>
                        <button
                          type="button"
                          className="employee-item__menu-dropdown-item employee-item__menu-dropdown-item--reinstate"
                          onClick={(e) => {
                            e.stopPropagation()
                            setOpenMenuId(null)
                            onReinstateEmployee(item.id)
                          }}
                          disabled={isReinstating}
                        >
                          {isReinstating ? t('ui.employeeList.reinstating') : t('ui.employeeList.reinstate')}
                        </button>
                        {onHandoverEmployee && (
                          <button
                            type="button"
                            className="employee-item__menu-dropdown-item employee-item__menu-dropdown-item--handover"
                            onClick={(e) => {
                              e.stopPropagation()
                              setOpenMenuId(null)
                              setHandoverConfirmId(item.id)
                            }}
                            disabled={isHandover}
                          >
                            {t('ui.employeeList.handover')}
                          </button>
                        )}
                        {onHardDeleteEmployee && (
                          <button
                            type="button"
                            className="employee-item__menu-dropdown-item employee-item__menu-dropdown-item--hard-delete"
                            onClick={(e) => {
                              e.stopPropagation()
                              setOpenMenuId(null)
                              setHardDeleteConfirmId(item.id)
                            }}
                            disabled={isHardDeleting}
                          >
                            {t('ui.employeeList.hardDelete')}
                          </button>
                        )}
                      </>
                    )}
                  </div>
                )}
              </div>
            </div>
          )
        })
      )}

      <ConfirmDialog
        open={fireConfirmId !== null}
        title={t('ui.employeeList.fireConfirm')}
        description={t('ui.employeeList.fireEmployeeName').replace('{name}', fireConfirmName)}
        confirmLabel={t('ui.employeeList.confirmFire')}
        cancelLabel={t('ui.employeeList.cancelFire')}
        onConfirm={() => {
          if (fireConfirmId) onFireEmployee(fireConfirmId)
          setFireConfirmId(null)
        }}
        onCancel={() => setFireConfirmId(null)}
        loading={fireConfirmId !== null && deletingEmployeeId === fireConfirmId}
      />

      <ConfirmDialog
        open={handoverConfirmId !== null}
        title={t('ui.employeeList.handoverConfirm')}
        description={t('ui.employeeList.handoverEmployeeName').replace('{name}', handoverConfirmName)}
        confirmLabel={t('ui.employeeList.confirmHandover')}
        cancelLabel={t('ui.employeeList.cancelHandover')}
        onConfirm={() => {
          if (handoverConfirmId) onHandoverEmployee?.(handoverConfirmId)
          setHandoverConfirmId(null)
        }}
        onCancel={() => setHandoverConfirmId(null)}
        loading={handoverConfirmId !== null && handoverEmployeeId === handoverConfirmId}
      />

      <ConfirmDialog
        open={hardDeleteConfirmId !== null}
        title={t('ui.employeeList.hardDeleteConfirm')}
        description={t('ui.employeeList.hardDeleteEmployeeName').replace('{name}', hardDeleteConfirmName)}
        confirmLabel={t('ui.employeeList.confirmHardDelete')}
        cancelLabel={t('ui.employeeList.cancelHardDelete')}
        onConfirm={() => {
          if (hardDeleteConfirmId) onHardDeleteEmployee?.(hardDeleteConfirmId)
          setHardDeleteConfirmId(null)
        }}
        onCancel={() => setHardDeleteConfirmId(null)}
        loading={hardDeleteConfirmId !== null && hardDeletingEmployeeId === hardDeleteConfirmId}
      />
    </div>
  )
}
