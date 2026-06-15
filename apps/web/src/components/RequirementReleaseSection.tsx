import React from 'react'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'

type RequirementReleaseSectionProps = {
  requirements: ReturnType<typeof useRequirementsWorkspace>
  t: (key: string) => string
}

export function RequirementReleaseSection({ requirements, t }: RequirementReleaseSectionProps) {
  const {
    detail,
    release,
    releaseLoading,
    reloadRelease,
  } = requirements

  React.useEffect(() => {
    if (detail) void reloadRelease(detail.id)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [detail?.id])

  return (
    <div className="requirement-development">
      <div className="requirement-release__artifacts">
        <h4 className="requirement-detail__label">{t('ui.requirements.release.artifactsTitle')}</h4>
        {release && release.artifacts.length > 0 ? (
          <ul className="requirement-release__artifact-list">
            {release.artifacts.map((name) => (
              <li key={name}>
                <code>{name}</code>
              </li>
            ))}
          </ul>
        ) : (
          <p className="settings-subtext">{t('ui.requirements.release.noArtifacts')}</p>
        )}
      </div>

      {release?.output ? (
        <div className="requirement-release__report">
          <h4 className="requirement-detail__label">{t('ui.requirements.release.outputTitle')}</h4>
          <pre className="requirement-release__pre">{release.output}</pre>
        </div>
      ) : null}

      {release?.run_log ? (
        <div className="requirement-release__report">
          <h4 className="requirement-detail__label">{t('ui.requirements.release.runLogTitle')}</h4>
          <pre className="requirement-release__pre">{release.run_log}</pre>
        </div>
      ) : null}
    </div>
  )
}
