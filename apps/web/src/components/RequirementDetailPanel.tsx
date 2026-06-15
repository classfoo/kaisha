import React from 'react'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'
import type { RequirementPhase } from '../features/requirements/requirementsApi'
import { RequirementPhaseTimeline } from './RequirementPhaseTimeline'
import { RequirementPhaseContent } from './RequirementPhaseContent'
import { RequirementActionBar } from './RequirementActionBar'

type RequirementDetailPanelProps = {
  requirements: ReturnType<typeof useRequirementsWorkspace>
  phaseLabel: (phase: RequirementPhase) => string
  t: (key: string) => string
}

export const RequirementDetailPanel = React.memo(function RequirementDetailPanel({ requirements, phaseLabel, t }: RequirementDetailPanelProps) {
  const { detail, loading, busy, error, saveRequirement } = requirements
  const [titleDraft, setTitleDraft] = React.useState('')
  const [phaseDraft, setPhaseDraft] = React.useState<RequirementPhase>('collection')
  const [viewPhase, setViewPhase] = React.useState<RequirementPhase>('collection')
  const [contentDraft, setContentDraft] = React.useState('')
  const [dirty, setDirty] = React.useState(false)
  const [saveError, setSaveError] = React.useState('')

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

  // Auto-save: debounce saves after user stops editing for 500ms
  const savingRef = React.useRef(false)
  React.useEffect(() => {
    if (!dirty || !detail) return
    const timer = setTimeout(async () => {
      if (savingRef.current) return
      savingRef.current = true
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
      } finally {
        savingRef.current = false
      }
    }, 500)
    return () => clearTimeout(timer)
  }, [dirty, detail?.id, titleDraft, phaseDraft, contentDraft, saveRequirement])

  const renderPhaseToolbar = () => {
    // Unified action bar for phase-specific actions (rendered below the timeline)
    return <RequirementActionBar requirements={requirements} viewPhase={viewPhase} t={t} />
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
            phase={detail.phase}
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
        {requirements.agentNotice ? (
          <div className="requirement-agent-notice">
            <span className="requirement-agent-notice__text">
              {t('ui.requirements.agentAssigned').replace('{name}', requirements.agentNotice.employee_name)}
            </span>
            <button
              className="requirement-agent-notice__close"
              onClick={() => requirements.clearAgentNotice?.()}
              aria-label={t('ui.requirements.dismissNotice') || 'Dismiss'}
              title={t('ui.requirements.dismissNotice') || 'Dismiss'}
            >
              ×
            </button>
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
})
