import React from 'react'
import type { RequirementPhase, RequirementSummary } from '../features/requirements/requirementsApi'
import { ConfirmDialog } from './ConfirmDialog'

type RequirementListProps = {
  items: RequirementSummary[]
  selectedId: string | null
  onSelect: (id: string) => void
  phaseLabel: (phase: RequirementPhase) => string
  t: (key: string) => string
  isArchivedView?: boolean
  onAbandonRequirement?: (id: string) => void
  onReinstateRequirement?: (id: string) => void
  onHardDeleteRequirement?: (id: string) => void
  abandoningId?: string | null
  reinstatingId?: string | null
  hardDeletingId?: string | null
}

export function RequirementList({
  items,
  selectedId,
  onSelect,
  phaseLabel,
  t,
  isArchivedView,
  onAbandonRequirement,
  onReinstateRequirement,
  onHardDeleteRequirement,
  abandoningId,
  reinstatingId,
  hardDeletingId,
}: RequirementListProps) {
  const [openMenuId, setOpenMenuId] = React.useState<string | null>(null)
  const [abandonConfirmId, setAbandonConfirmId] = React.useState<string | null>(null)
  const [hardDeleteConfirmId, setHardDeleteConfirmId] = React.useState<string | null>(null)

  const abandonConfirmTitle = abandonConfirmId
    ? items.find((r) => r.id === abandonConfirmId)?.title ?? ''
    : ''
  const hardDeleteConfirmTitle = hardDeleteConfirmId
    ? items.find((r) => r.id === hardDeleteConfirmId)?.title ?? ''
    : ''

  return (
    <div className="employee-list requirement-list" role="listbox" aria-label={t('ui.requirements.listTitle')}>
      {items.length === 0 ? (
        <div className="employee-list__empty">
          {isArchivedView
            ? t('ui.requirements.archivedEmpty')
            : t('ui.requirements.empty')}
        </div>
      ) : (
        items.map((item) => {
          const isActive = item.id === selectedId
          const isAbandoning = abandoningId === item.id
          const isReinstating = reinstatingId === item.id
          const isHardDeleting = hardDeletingId === item.id
          const isBusy = isAbandoning || isReinstating || isHardDeleting
          const menuOpen = openMenuId === item.id
          return (
            <div
              key={item.id}
              className={`employee-item requirement-item ${isActive ? 'employee-item--active' : ''} ${isArchivedView ? 'employee-item--archived' : ''} ${menuOpen ? 'employee-item--menu-open' : ''}`}
            >
              <button
                type="button"
                className="employee-item__body"
                onClick={() => onSelect(item.id)}
                disabled={isBusy}
              >
                <div className="employee-item__avatar requirement-item__phase">
                  {phaseLabel(item.phase).slice(0, 1)}
                </div>
                <div className="employee-item__main">
                  <div className="employee-item__name">{item.title}</div>
                  <div className="employee-item__snippet requirement-item__phase-label">
                    {phaseLabel(item.phase)}
                  </div>
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
                    {!isArchivedView && onAbandonRequirement && (
                      <button
                        type="button"
                        className="employee-item__menu-dropdown-item employee-item__menu-dropdown-item--fire"
                        onClick={(e) => {
                          e.stopPropagation()
                          setOpenMenuId(null)
                          setAbandonConfirmId(item.id)
                        }}
                        disabled={isAbandoning}
                      >
                        {t('ui.requirements.abandon')}
                      </button>
                    )}
                    {isArchivedView && (
                      <>
                        {onReinstateRequirement && (
                          <button
                            type="button"
                            className="employee-item__menu-dropdown-item employee-item__menu-dropdown-item--reinstate"
                            onClick={(e) => {
                              e.stopPropagation()
                              setOpenMenuId(null)
                              onReinstateRequirement(item.id)
                            }}
                            disabled={isReinstating}
                          >
                            {isReinstating ? t('ui.requirements.reinstating') : t('ui.requirements.reinstate')}
                          </button>
                        )}
                        {onHardDeleteRequirement && (
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
                            {t('ui.requirements.hardDelete')}
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
        open={abandonConfirmId !== null}
        title={t('ui.requirements.abandonConfirm')}
        description={t('ui.requirements.abandonRequirementName').replace('{name}', abandonConfirmTitle)}
        confirmLabel={t('ui.requirements.confirmAbandon')}
        cancelLabel={t('ui.requirements.cancelAbandon')}
        onConfirm={() => {
          if (abandonConfirmId) onAbandonRequirement?.(abandonConfirmId)
          setAbandonConfirmId(null)
        }}
        onCancel={() => setAbandonConfirmId(null)}
        loading={abandonConfirmId !== null && abandoningId === abandonConfirmId}
      />

      <ConfirmDialog
        open={hardDeleteConfirmId !== null}
        title={t('ui.requirements.hardDeleteConfirm')}
        description={t('ui.requirements.hardDeleteRequirementName').replace('{name}', hardDeleteConfirmTitle)}
        confirmLabel={t('ui.requirements.confirmHardDelete')}
        cancelLabel={t('ui.requirements.cancelHardDelete')}
        onConfirm={() => {
          if (hardDeleteConfirmId) onHardDeleteRequirement?.(hardDeleteConfirmId)
          setHardDeleteConfirmId(null)
        }}
        onCancel={() => setHardDeleteConfirmId(null)}
        loading={hardDeleteConfirmId !== null && hardDeletingId === hardDeleteConfirmId}
      />
    </div>
  )
}
