import React from 'react'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'

type RequirementConfirmSectionProps = {
  requirements: ReturnType<typeof useRequirementsWorkspace>
  t: (key: string) => string
}

function confirmStatusLabel(t: (key: string) => string, status: string | undefined) {
  if (status === 'confirmed') return t('ui.requirements.confirm.statusConfirmed')
  if (status === 'abandoned') return t('ui.requirements.confirm.statusAbandoned')
  return t('ui.requirements.confirm.statusPending')
}

export function RequirementConfirmSection({ requirements, t }: RequirementConfirmSectionProps) {
  const { detail } = requirements
  const status = detail?.confirm_status

  return (
    <div className="requirement-confirm">
      <div className="requirement-confirm__status">
        <span className="settings-subtext">{t('ui.requirements.confirm.statusLabel')}</span>
        <span
          className={`requirement-confirm__badge requirement-confirm__badge--${status ?? 'pending'}`}
        >
          {confirmStatusLabel(t, status)}
        </span>
      </div>
    </div>
  )
}
