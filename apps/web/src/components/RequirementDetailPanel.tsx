import React from 'react'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'
import type { RequirementPhase } from '../features/requirements/requirementsApi'
import { RequirementPhaseTimeline } from './RequirementPhaseTimeline'
import { RequirementPhaseContent } from './RequirementPhaseContent'

type RequirementDetailPanelProps = {
  requirements: ReturnType<typeof useRequirementsWorkspace>
  phaseLabel: (phase: RequirementPhase) => string
  t: (key: string) => string
}

export function RequirementDetailPanel({ requirements, phaseLabel, t }: RequirementDetailPanelProps) {
  const { detail, loading, busy, error, saveRequirement, runReview, reviewRunning, reviewForcePassing, confirming, abandoning, reconfirming, confirmRequirement, abandonRequirement, reconfirmRequirement } = requirements
  const [titleDraft, setTitleDraft] = React.useState('')
  const [phaseDraft, setPhaseDraft] = React.useState<RequirementPhase>('collection')
  const [viewPhase, setViewPhase] = React.useState<RequirementPhase>('collection')
  const [contentDraft, setContentDraft] = React.useState('')
  const [dirty, setDirty] = React.useState(false)
  const [saveError, setSaveError] = React.useState('')
  const [confirmReviewOpen, setConfirmReviewOpen] = React.useState(false)
  const [reviewError, setReviewError] = React.useState('')
  const [confirmConfirmOpen, setConfirmConfirmOpen] = React.useState(false)
  const [confirmActionError, setConfirmActionError] = React.useState('')
  const [confirmAbandonOpen, setConfirmAbandonOpen] = React.useState(false)
  const [abandonActionError, setAbandonActionError] = React.useState('')
  const [confirmReconfirmOpen, setConfirmReconfirmOpen] = React.useState(false)
  const [reconfirmActionError, setReconfirmActionError] = React.useState('')
  const [confirmForceOpen, setConfirmForceOpen] = React.useState(false)
  const [forceError, setForceError] = React.useState('')

  React.useEffect(() => {
    if (!detail) {
      setTitleDraft('')
      setPhaseDraft('collection')
      setViewPhase('collection')
      setContentDraft('')
      setDirty(false)
      return
    }
    setTitleDraft(detail.title)
    setPhaseDraft(detail.phase)
    setViewPhase(detail.phase)
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

  const currentPhase = detail?.phase ?? phaseDraft

  const renderPhaseToolbar = () => {
    const phase = currentPhase

    // collection phase: save button
    if (phase === 'collection') {
      return (
        <div className="requirement-phase-toolbar">
          <button
            type="button"
            className="action-btn"
            onClick={() => void onSave()}
            disabled={busy || !dirty}
          >
            {busy ? t('ui.requirements.saving') : t('ui.requirements.save')}
          </button>
        </div>
      )
    }

    // review phase: enter review, force pass
    if (phase === 'review') {
      return (
        <div className="requirement-phase-toolbar">
          <button
            type="button"
            className="action-btn"
            onClick={() => setConfirmReviewOpen(true)}
            disabled={busy || reviewRunning}
          >
            {reviewRunning ? t('ui.requirements.review.running') : t('ui.requirements.review.enter')}
          </button>
          {!reviewRunning && (
            <button
              type="button"
              className="action-btn"
              onClick={() => setConfirmForceOpen(true)}
              disabled={busy || reviewForcePassing}
            >
              {reviewForcePassing ? t('ui.requirements.review.forcePassing') : t('ui.requirements.review.forcePass')}
            </button>
          )}
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
                      .then(() => {
                        setConfirmReviewOpen(false)
                        setViewPhase('review')
                      })
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
          {confirmForceOpen ? (
            <div className="requirement-review-confirm" role="dialog" aria-modal="true">
              <p className="requirement-review-confirm__text">{t('ui.requirements.review.forcePassConfirmText')}</p>
              <div className="requirement-review-confirm__actions">
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    setConfirmForceOpen(false)
                    setForceError('')
                  }}
                >
                  {t('ui.requirements.review.cancel')}
                </button>
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    if (!detail) return
                    setForceError('')
                    void requirements.forcePassReview(detail.id)
                      .then(() => {
                        setConfirmForceOpen(false)
                      })
                      .catch((e) => setForceError(e instanceof Error ? e.message : String(e)))
                  }}
                  disabled={reviewForcePassing}
                >
                  {t('ui.requirements.review.forcePassConfirm')}
                </button>
              </div>
              {forceError ? <p className="workspace-setup__error">{forceError}</p> : null}
            </div>
          ) : null}
        </div>
      )
    }

    // confirm phase: confirm, abandon, reconfirm based on status
    if (phase === 'confirm') {
      const confirmStatus = detail?.confirm_status
      const isAbandoned = confirmStatus === 'abandoned'
      const isConfirmed = confirmStatus === 'confirmed'

      return (
        <div className="requirement-phase-toolbar">
          {!isConfirmed && !isAbandoned && (
            <>
              <button
                type="button"
                className="action-btn"
                onClick={() => setConfirmConfirmOpen(true)}
                disabled={busy || confirming}
              >
                {confirming ? t('ui.requirements.confirm.processing') : t('ui.requirements.confirm.confirmAction')}
              </button>
              <button
                type="button"
                className="action-btn"
                onClick={() => setConfirmAbandonOpen(true)}
                disabled={busy || abandoning}
              >
                {abandoning ? t('ui.requirements.confirm.processing') : t('ui.requirements.confirm.abandonAction')}
              </button>
            </>
          )}
          {isAbandoned && (
            <button
              type="button"
              className="action-btn"
              onClick={() => setConfirmReconfirmOpen(true)}
              disabled={busy || reconfirming}
            >
              {reconfirming ? t('ui.requirements.confirm.processing') : t('ui.requirements.confirm.reconfirmAction')}
            </button>
          )}
          {isConfirmed && (
            <button type="button" className="action-btn" disabled>
              {t('ui.requirements.confirm.statusConfirmed')}
            </button>
          )}
          {confirmConfirmOpen ? (
            <div className="requirement-review-confirm" role="dialog" aria-modal="true">
              <p className="requirement-review-confirm__text">{t('ui.requirements.confirm.confirmDialogText')}</p>
              <div className="requirement-review-confirm__actions">
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    setConfirmConfirmOpen(false)
                    setConfirmActionError('')
                  }}
                >
                  {t('ui.requirements.review.cancel')}
                </button>
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    if (!detail) return
                    setConfirmActionError('')
                    void confirmRequirement(detail.id)
                      .then(() => {
                        setConfirmConfirmOpen(false)
                        setViewPhase(detail.phase === 'confirm' ? 'development' : detail.phase)
                      })
                      .catch((e) => setConfirmActionError(e instanceof Error ? e.message : String(e)))
                  }}
                  disabled={confirming}
                >
                  {t('ui.requirements.confirm.confirmDialogTitle')}
                </button>
              </div>
              {confirmActionError ? <p className="workspace-setup__error">{confirmActionError}</p> : null}
            </div>
          ) : null}
          {confirmAbandonOpen ? (
            <div className="requirement-review-confirm" role="dialog" aria-modal="true">
              <p className="requirement-review-confirm__text">{t('ui.requirements.confirm.abandonDialogText')}</p>
              <div className="requirement-review-confirm__actions">
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    setConfirmAbandonOpen(false)
                    setAbandonActionError('')
                  }}
                >
                  {t('ui.requirements.review.cancel')}
                </button>
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    if (!detail) return
                    setAbandonActionError('')
                    void abandonRequirement(detail.id)
                      .then(() => setConfirmAbandonOpen(false))
                      .catch((e) => setAbandonActionError(e instanceof Error ? e.message : String(e)))
                  }}
                  disabled={abandoning}
                >
                  {t('ui.requirements.confirm.abandonDialogTitle')}
                </button>
              </div>
              {abandonActionError ? <p className="workspace-setup__error">{abandonActionError}</p> : null}
            </div>
          ) : null}
          {confirmReconfirmOpen ? (
            <div className="requirement-review-confirm" role="dialog" aria-modal="true">
              <p className="requirement-review-confirm__text">{t('ui.requirements.confirm.reconfirmDialogText')}</p>
              <div className="requirement-review-confirm__actions">
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    setConfirmReconfirmOpen(false)
                    setReconfirmActionError('')
                  }}
                >
                  {t('ui.requirements.review.cancel')}
                </button>
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    if (!detail) return
                    setReconfirmActionError('')
                    void reconfirmRequirement(detail.id)
                      .then(() => {
                        setConfirmReconfirmOpen(false)
                      })
                      .catch((e) => setReconfirmActionError(e instanceof Error ? e.message : String(e)))
                  }}
                  disabled={reconfirming}
                >
                  {t('ui.requirements.confirm.reconfirmDialogTitle')}
                </button>
              </div>
              {reconfirmActionError ? <p className="workspace-setup__error">{reconfirmActionError}</p> : null}
            </div>
          ) : null}
        </div>
      )
    }

    // development, testing, release: placeholder toolbar (save button)
    return (
      <div className="requirement-phase-toolbar">
        <button
          type="button"
          className="action-btn"
          onClick={() => void onSave()}
          disabled={busy || !dirty}
        >
          {busy ? t('ui.requirements.saving') : t('ui.requirements.save')}
        </button>
      </div>
    )
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
        <section className="requirement-detail__timeline-wrap">
          <RequirementPhaseTimeline
            phase={phaseDraft}
            viewPhase={viewPhase}
            phaseLabel={phaseLabel}
            disabled={busy}
            onViewPhaseChange={(next) => {
              setViewPhase(next)
              setPhaseDraft(next)
              markDirty()
            }}
          />
        </section>
        {renderPhaseToolbar()}
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
      <RequirementPhaseContent
        viewPhase={viewPhase}
        phaseLabel={phaseLabel}
        contentDraft={contentDraft}
        onContentChange={(value) => {
          setContentDraft(value)
          markDirty()
        }}
        requirements={requirements}
        t={t}
      />
    </div>
  )
}
