import React from 'react'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'
import type { RequirementPhase } from '../features/requirements/requirementsApi'
import { phaseViewKind } from '../features/requirements/requirementPhaseView'
import { RequirementReviewSection } from './RequirementReviewSection'

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
  const { detail, loadReview } = requirements
  const kind = phaseViewKind(viewPhase)

  React.useEffect(() => {
    if (!detail || kind !== 'review') return
    void loadReview(detail.id).catch(() => undefined)
  }, [detail?.id, kind, loadReview])

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

  if (kind === 'review') {
    return (
      <section className="requirement-detail__stage requirement-detail__stage--review">
        <h3 className="requirement-detail__label">{t('ui.requirements.review.sectionTitle')}</h3>
        <RequirementReviewSection requirements={requirements} t={t} />
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
