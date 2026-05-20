import React from 'react'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'
import type { RequirementPhase } from '../features/requirements/requirementsApi'
import { RequirementPhaseTimeline } from './RequirementPhaseTimeline'
import { RequirementReviewSection } from './RequirementReviewSection'

type RequirementDetailPanelProps = {
  requirements: ReturnType<typeof useRequirementsWorkspace>
  phaseLabel: (phase: RequirementPhase) => string
  t: (key: string) => string
}

export function RequirementDetailPanel({ requirements, phaseLabel, t }: RequirementDetailPanelProps) {
  const { detail, loading, busy, error, saveRequirement, runReview, reviewRunning } = requirements
  const [titleDraft, setTitleDraft] = React.useState('')
  const [phaseDraft, setPhaseDraft] = React.useState<RequirementPhase>('collection')
  const [contentDraft, setContentDraft] = React.useState('')
  const [dirty, setDirty] = React.useState(false)
  const [saveError, setSaveError] = React.useState('')
  const [confirmReviewOpen, setConfirmReviewOpen] = React.useState(false)
  const [reviewError, setReviewError] = React.useState('')

  React.useEffect(() => {
    if (!detail) {
      setTitleDraft('')
      setPhaseDraft('collection')
      setContentDraft('')
      setDirty(false)
      return
    }
    setTitleDraft(detail.title)
    setPhaseDraft(detail.phase)
    setContentDraft(detail.content)
    setDirty(false)
    setSaveError('')
  }, [detail?.id, detail?.title, detail?.phase, detail?.content])

  const markDirty = () => setDirty(true)

  const onSave = async () => {
    if (!detail) return
    setSaveError('')
    try {
      await saveRequirement({
        title: titleDraft.trim(),
        phase: phaseDraft,
        content: contentDraft,
      })
      setDirty(false)
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : String(e))
    }
  }

  if (loading && !detail) {
    return <div className="requirement-detail requirement-detail--empty">{t('ui.requirements.loading')}</div>
  }

  if (!detail) {
    return <div className="requirement-detail requirement-detail--empty">{t('ui.requirements.selectHint')}</div>
  }

  return (
    <div className="requirement-detail">
      <header className="requirement-detail__header">
        <input
          className="requirement-detail__title"
          value={titleDraft}
          onChange={(e) => {
            setTitleDraft(e.target.value)
            markDirty()
          }}
          aria-label={t('ui.requirements.titleLabel')}
        />
        <section className="requirement-detail__timeline-wrap" aria-labelledby="requirement-phase-heading">
          <h3 id="requirement-phase-heading" className="requirement-detail__label">
            {t('ui.requirements.phaseLabel')}
          </h3>
          <RequirementPhaseTimeline
            phase={phaseDraft}
            phaseLabel={phaseLabel}
            disabled={busy}
            onPhaseChange={(next) => {
              setPhaseDraft(next)
              markDirty()
            }}
            t={t}
          />
        </section>
        <div className="requirement-detail__actions">
          <button
            type="button"
            className="action-btn"
            onClick={() => setConfirmReviewOpen(true)}
            disabled={busy || reviewRunning}
          >
            {reviewRunning ? t('ui.requirements.review.running') : t('ui.requirements.review.enter')}
          </button>
          <button
            type="button"
            className="action-btn"
            onClick={() => void onSave()}
            disabled={busy || !dirty}
          >
            {busy ? t('ui.requirements.saving') : t('ui.requirements.save')}
          </button>
        </div>
        {confirmReviewOpen ? (
          <div className="requirement-review-confirm" role="dialog" aria-modal="true">
            <p className="requirement-review-confirm__text">{t('ui.requirements.review.confirmText')}</p>
            <div className="requirement-review-confirm__actions">
              <button
                type="button"
                className="action-btn"
                onClick={() => {
                  setConfirmReviewOpen(false)
                  setReviewError('')
                }}
              >
                {t('ui.requirements.review.cancel')}
              </button>
              <button
                type="button"
                className="action-btn"
                onClick={() => {
                  if (!detail) return
                  setReviewError('')
                  void runReview(detail.id)
                    .then(() => setConfirmReviewOpen(false))
                    .catch((e) => setReviewError(e instanceof Error ? e.message : String(e)))
                }}
                disabled={reviewRunning}
              >
                {t('ui.requirements.review.confirm')}
              </button>
            </div>
            {reviewError ? <p className="workspace-setup__error">{reviewError}</p> : null}
          </div>
        ) : null}
        <div className="requirement-detail__path">
          <span className="settings-subtext">{t('ui.requirements.dirLabel')}</span>
          <code className="requirement-detail__dir">{detail.dir_path}</code>
        </div>
        {detail.subdirs.length > 0 ? (
          <div className="requirement-detail__subdirs">
            <span className="settings-subtext">{t('ui.requirements.subdirsLabel')}</span>
            <span className="requirement-detail__subdir-list">{detail.subdirs.join(', ')}</span>
          </div>
        ) : null}
        {error || saveError ? (
          <p className="workspace-setup__error">{saveError || error}</p>
        ) : null}
      </header>
      <section className="requirement-detail__review">
        <h3 className="requirement-detail__label">{t('ui.requirements.review.sectionTitle')}</h3>
        <RequirementReviewSection requirements={requirements} t={t} />
      </section>
      <section className="requirement-detail__body">
        <label className="requirement-detail__label" htmlFor="requirement-content">
          {t('ui.requirements.contentLabel')}
        </label>
        <textarea
          id="requirement-content"
          className="requirement-detail__editor"
          value={contentDraft}
          onChange={(e) => {
            setContentDraft(e.target.value)
            markDirty()
          }}
          placeholder={t('ui.requirements.contentPlaceholder')}
        />
      </section>
    </div>
  )
}
