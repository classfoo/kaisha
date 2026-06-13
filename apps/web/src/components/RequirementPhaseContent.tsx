import React from 'react'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'
import type { RequirementPhase } from '../features/requirements/requirementsApi'
import { phaseViewKind } from '../features/requirements/requirementPhaseView'
import { RequirementDevelopmentSection } from './RequirementDevelopmentSection'

type RequirementPhaseContentProps = {
  viewPhase: RequirementPhase
  phaseLabel: (phase: RequirementPhase) => string
  contentDraft: string
  onContentChange: (value: string) => void
  requirements: ReturnType<typeof useRequirementsWorkspace>
  t: (key: string) => string
}

export function RequirementPhaseContent({
  viewPhase,
  phaseLabel,
  contentDraft,
  onContentChange,
  requirements,
  t,
}: RequirementPhaseContentProps) {
  const kind = phaseViewKind(viewPhase)

  if (kind === 'collection') {
    return (
      <section className="requirement-detail__body">
        <label className="requirement-detail__label" htmlFor="requirement-content">
          {t('ui.requirements.contentLabel')}
        </label>
        <textarea
          id="requirement-content"
          className="requirement-detail__editor"
          value={contentDraft}
          onChange={(e) => onContentChange(e.target.value)}
          placeholder={t('ui.requirements.contentPlaceholder')}
        />
      </section>
    )
  }

  if (kind === 'development') {
    return (
      <section className="requirement-detail__stage requirement-detail__stage--development">
        <h3 className="requirement-detail__label">{t('ui.requirements.development.sectionTitle')}</h3>
        <RequirementDevelopmentSection requirements={requirements} t={t} />
      </section>
    )
  }

  return (
    <section className="requirement-detail__stage requirement-detail__stage--placeholder">
      <h3 className="requirement-detail__label">{phaseLabel(viewPhase)}</h3>
      <p className="settings-subtext">{t(`ui.requirements.phaseViews.${viewPhase}.empty`)}</p>
    </section>
  )
}
