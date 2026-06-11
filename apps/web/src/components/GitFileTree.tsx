import React from 'react'
import type { GitTreeEntry, GitTreeListing } from '../features/git/gitApi'

type ListTree = (path: string) => Promise<GitTreeListing | null>

type GitFileTreeProps = {
  listTree: ListTree
  onOpenFile: (path: string, name: string) => void
  reloadKey: number
  t: (key: string) => string
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}

function GitTreeNode({
  entry,
  depth,
  listTree,
  onOpenFile,
  t,
}: {
  entry: GitTreeEntry
  depth: number
  listTree: ListTree
  onOpenFile: (path: string, name: string) => void
  t: (key: string) => string
}) {
  const [expanded, setExpanded] = React.useState(false)
  const [children, setChildren] = React.useState<GitTreeEntry[] | null>(null)
  const [loading, setLoading] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  const indent = { paddingLeft: `${depth * 14 + 8}px` }

  const toggle = async () => {
    if (!entry.is_dir) return
    const next = !expanded
    setExpanded(next)
    if (next && children === null) {
      setLoading(true)
      setError(null)
      try {
        const res = await listTree(entry.path)
        setChildren(res?.entries ?? [])
      } catch (e) {
        setError(errorText(e))
      } finally {
        setLoading(false)
      }
    }
  }

  if (entry.is_dir) {
    return (
      <li className="git-tree__node">
        <button type="button" className="git-tree__row git-tree__row--dir" style={indent} onClick={toggle}>
          <span className="git-tree__chevron">{expanded ? '▾' : '▸'}</span>
          <span className="git-tree__icon" aria-hidden="true">📁</span>
          <span className="git-tree__name">{entry.name}</span>
        </button>
        {expanded ? (
          loading ? (
            <div className="git-tree__state" style={{ paddingLeft: `${depth * 14 + 28}px` }}>
              {t('ui.git.tree.loading')}
            </div>
          ) : error ? (
            <div
              className="git-tree__state git-tree__state--error"
              style={{ paddingLeft: `${depth * 14 + 28}px` }}
            >
              {error}
            </div>
          ) : (children?.length ?? 0) === 0 ? (
            <div className="git-tree__state" style={{ paddingLeft: `${depth * 14 + 28}px` }}>
              {t('ui.git.tree.emptyDir')}
            </div>
          ) : (
            <ul className="git-tree__children">
              {children!.map((child) => (
                <GitTreeNode
                  key={child.path}
                  entry={child}
                  depth={depth + 1}
                  listTree={listTree}
                  onOpenFile={onOpenFile}
                  t={t}
                />
              ))}
            </ul>
          )
        ) : null}
      </li>
    )
  }

  return (
    <li className="git-tree__node">
      <button
        type="button"
        className="git-tree__row git-tree__row--file"
        style={indent}
        onDoubleClick={() => onOpenFile(entry.path, entry.name)}
        title={t('ui.git.tree.openHint')}
      >
        <span className="git-tree__chevron" aria-hidden="true" />
        <span className="git-tree__icon" aria-hidden="true">📄</span>
        <span className="git-tree__name">{entry.name}</span>
        <span className="git-tree__size">{formatSize(entry.size)}</span>
      </button>
    </li>
  )
}

export function GitFileTree({ listTree, onOpenFile, reloadKey, t }: GitFileTreeProps) {
  const [entries, setEntries] = React.useState<GitTreeEntry[]>([])
  const [loading, setLoading] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  React.useEffect(() => {
    let cancelled = false
    setLoading(true)
    setError(null)
    listTree('')
      .then((res) => {
        if (cancelled) return
        setEntries(res?.entries ?? [])
      })
      .catch((e) => {
        if (cancelled) return
        setError(errorText(e))
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [listTree, reloadKey])

  if (loading) {
    return <div className="git-tree__state">{t('ui.git.tree.loading')}</div>
  }
  if (error) {
    return <div className="git-tree__state git-tree__state--error">{error}</div>
  }
  if (entries.length === 0) {
    return <div className="git-tree__state">{t('ui.git.tree.empty')}</div>
  }

  return (
    <ul className="git-tree">
      {entries.map((entry) => (
        <GitTreeNode
          key={entry.path}
          entry={entry}
          depth={0}
          listTree={listTree}
          onOpenFile={onOpenFile}
          t={t}
        />
      ))}
    </ul>
  )
}
