import React from 'react'
import { createPortal } from 'react-dom'
import type { GitFileContent } from '../features/git/gitApi'

type GitFileDialogProps = {
  open: boolean
  loading: boolean
  error: string | null
  title: string
  file: GitFileContent | null
  t: (key: string) => string
  onClose: () => void
}

export function GitFileDialog({ open, loading, error, title, file, t, onClose }: GitFileDialogProps) {
  React.useEffect(() => {
    if (!open) return
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', handleKey)
    return () => {
      document.body.style.overflow = previousOverflow
      document.removeEventListener('keydown', handleKey)
    }
  }, [open, onClose])

  if (!open) return null

  return createPortal(
    <div className="git-file-dialog" role="dialog" aria-modal="true" aria-label={title || t('ui.git.file.title')}>
      <div className="git-file-dialog__overlay" onClick={onClose} />
      <div className="git-file-dialog__content">
        <div className="git-file-dialog__header">
          <h3 className="git-file-dialog__title">{title || t('ui.git.file.title')}</h3>
          <button
            type="button"
            className="git-file-dialog__close"
            onClick={onClose}
            aria-label={t('ui.git.file.close')}
          >
            ×
          </button>
        </div>

        {loading ? (
          <div className="git-file-dialog__state">{t('ui.git.file.loading')}</div>
        ) : error ? (
          <div className="git-file-dialog__state git-file-dialog__state--error">{error}</div>
        ) : file ? (
          file.binary ? (
            <div className="git-file-dialog__state">{t('ui.git.file.binary')}</div>
          ) : (
            <div className="git-file-dialog__body">
              {file.truncated ? (
                <div className="git-file-dialog__notice">{t('ui.git.file.truncated')}</div>
              ) : null}
              {file.content.trim().length === 0 ? (
                <div className="git-file-dialog__state">{t('ui.git.file.emptyFile')}</div>
              ) : (
                <pre className="git-file-dialog__code">{file.content}</pre>
              )}
            </div>
          )
        ) : null}

        <div className="git-file-dialog__actions">
          <button type="button" className="git-file-dialog__btn" onClick={onClose}>
            {t('ui.git.file.close')}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  )
}
