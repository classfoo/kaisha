import React from 'react'

const API_BASE = import.meta.env.VITE_API_BASE ?? 'http://127.0.0.1:8080'

export default function App() {
  const [status, setStatus] = React.useState('checking...')

  React.useEffect(() => {
    fetch(`${API_BASE}/api/health`)
      .then((res) => res.json())
      .then((json) => setStatus(json.status ?? 'unknown'))
      .catch(() => setStatus('offline'))
  }, [])

  return (
    <div className="app-shell">
      <aside className="side-panel">
        <div className="side-panel__brand">Codeband</div>
        <nav className="side-panel__nav">
          <button className="nav-item nav-item--active">Workspace</button>
          <button className="nav-item">Explorer</button>
          <button className="nav-item">Search</button>
          <button className="nav-item">Settings</button>
        </nav>
        <div className="side-panel__footer">
          <span>Backend</span>
          <span className={`status status--${status}`}>{status}</span>
        </div>
      </aside>

      <section className="work-area">
        <header className="work-area__topbar">
          <div className="topbar__title">Current Project</div>
          <div className="topbar__actions">
            <button className="action-btn">Run</button>
            <button className="action-btn">Share</button>
          </div>
        </header>

        <main className="work-area__content">
          <div className="content-placeholder">
            Right workspace area
          </div>
        </main>
      </section>
    </div>
  )
}
