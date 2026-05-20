import React from 'react'
import { createRequirementsApi, type WorkRules } from '../features/requirements/requirementsApi'

type WorkRulesSettingsProps = {
  apiBase: string
  locale: string
  t: (key: string) => string
}

export function WorkRulesSettings({ apiBase, locale, t }: WorkRulesSettingsProps) {
  const api = React.useMemo(() => createRequirementsApi(apiBase, locale), [apiBase, locale])
  const [rules, setRules] = React.useState<WorkRules | null>(null)
  const [loading, setLoading] = React.useState(true)
  const [saving, setSaving] = React.useState(false)
  const [error, setError] = React.useState('')
  const [saved, setSaved] = React.useState(false)

  React.useEffect(() => {
    setLoading(true)
    setError('')
    api
      .getWorkRules()
      .then(setRules)
      .catch(() => setError(t('ui.workRules.loadError')))
      .finally(() => setLoading(false))
  }, [api, t])

  const updateRole = (key: string, patch: Partial<WorkRules['roles'][string]>) => {
    setRules((prev) => {
      if (!prev) return prev
      const role = prev.roles[key]
      if (!role) return prev
      return {
        ...prev,
        roles: {
          ...prev.roles,
          [key]: { ...role, ...patch },
        },
      }
    })
    setSaved(false)
  }

  const onSave = async () => {
    if (!rules) return
    setSaving(true)
    setError('')
    try {
      await api.saveWorkRules(rules)
      setSaved(true)
    } catch {
      setError(t('ui.workRules.saveError'))
    } finally {
      setSaving(false)
    }
  }

  if (loading) {
    return <p className="settings-empty">{t('ui.requirements.loading')}</p>
  }

  if (!rules) {
    return <p className="settings-empty">{error || t('ui.workRules.loadError')}</p>
  }

  return (
    <>
      <section className="settings-card">
        <h3 className="settings-card__title">{t('ui.workRules.title')}</h3>
        <p className="settings-subtext">{t('ui.workRules.hint')}</p>
        {Object.entries(rules.roles).map(([key, role]) => (
          <div key={key} className="work-rules-role">
            <div className="work-rules-role__head">
              <strong>{role.display_name}</strong>
              <span className="settings-subtext">{key}</span>
            </div>
            <label className="requirement-detail__label">{t('ui.workRules.displayName')}</label>
            <input
              className="settings-input"
              value={role.display_name}
              onChange={(e) => updateRole(key, { display_name: e.target.value })}
            />
            <label className="requirement-detail__label">{t('ui.workRules.aliases')}</label>
            <input
              className="settings-input"
              value={role.aliases.join(', ')}
              onChange={(e) =>
                updateRole(key, {
                  aliases: e.target.value.split(',').map((s) => s.trim()).filter(Boolean),
                })
              }
            />
            <label className="requirement-detail__label">{t('ui.workRules.dutyReview')}</label>
            <textarea
              className="requirement-detail__editor work-rules-role__duty"
              value={role.duties.review ?? ''}
              onChange={(e) =>
                updateRole(key, {
                  duties: { ...role.duties, review: e.target.value },
                })
              }
            />
          </div>
        ))}
        <button type="button" className="action-btn" onClick={() => void onSave()} disabled={saving}>
          {saving ? t('ui.workRules.saving') : t('ui.workRules.save')}
        </button>
        {saved ? <p className="settings-subtext">{t('ui.workRules.saved')}</p> : null}
        {error ? <p className="workspace-setup__error">{error}</p> : null}
      </section>
    </>
  )
}
