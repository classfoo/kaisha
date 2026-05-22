import React from 'react'
import { MacWindowControls } from './MacWindowControls'

export type NavMenu = 'home' | 'chat' | 'git' | 'requirements'

type LeftSidebarProps = {
  activeNav: NavMenu
  topNavItems: { id: NavMenu; labelKey: string }[]
  bottomNavItems: { id: 'settings'; labelKey: string }[]
  settingsOpen: boolean
  t: (key: string) => string
  onMenuClick: (menu: NavMenu) => void
  onSettingsClick: () => void
}

function SidebarIcon({ menu }: { menu: NavMenu }) {
  if (menu === 'home') {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true" className="left-rail__icon-svg">
        <path d="M4 11.5L12 5l8 6.5V20h-5.5v-5h-5v5H4z" fill="none" stroke="currentColor" strokeWidth="1.8" />
      </svg>
    )
  }
  if (menu === 'chat') {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true" className="left-rail__icon-svg">
        <path d="M5 6h14v10H9l-4 3V6z" fill="none" stroke="currentColor" strokeWidth="1.8" />
      </svg>
    )
  }
  if (menu === 'git') {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true" className="left-rail__icon-svg">
        <path
          d="M6 4h6l2 2h6v14H6V4zm3 10a2 2 0 1 0 0-4 2 2 0 0 0 0 4zm6 4a2 2 0 1 0 0-4 2 2 0 0 0 0 4z"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.8"
        />
      </svg>
    )
  }
  if (menu === 'requirements') {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true" className="left-rail__icon-svg">
        <path
          d="M7 4h10v3H7V4zm0 6h10v3H7v-3zm0 6h7v3H7v-3z"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.8"
        />
      </svg>
    )
  }
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" className="left-rail__icon-svg">
      <path d="M12 4l7 4v8l-7 4-7-4V8zM12 4v16M5 8l7 4 7-4" fill="none" stroke="currentColor" strokeWidth="1.8" />
    </svg>
  )
}

export function LeftSidebar({
  activeNav,
  topNavItems,
  bottomNavItems,
  settingsOpen,
  t,
  onMenuClick,
  onSettingsClick,
}: LeftSidebarProps) {
  return (
    <aside className="left-rail" data-tauri-drag-region>
      <div className="left-rail__window-controls" onMouseDown={(event) => event.stopPropagation()}>
        <MacWindowControls t={t} />
      </div>
      <nav className="left-rail__menu left-rail__menu--top" onMouseDown={(event) => event.stopPropagation()}>
        {topNavItems.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`left-rail__btn ${activeNav === item.id ? 'left-rail__btn--active' : ''}`}
            aria-label={t(item.labelKey)}
            title={t(item.labelKey)}
            onClick={() => onMenuClick(item.id)}
          >
            <span className="left-rail__icon">
              <SidebarIcon menu={item.id} />
            </span>
          </button>
        ))}
      </nav>
      <nav className="left-rail__menu left-rail__menu--bottom" onMouseDown={(event) => event.stopPropagation()}>
        {bottomNavItems.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`left-rail__btn ${settingsOpen ? 'left-rail__btn--active' : ''}`}
            aria-label={t(item.labelKey)}
            title={t(item.labelKey)}
            onClick={onSettingsClick}
          >
            <span className="left-rail__icon">
              <svg viewBox="0 0 24 24" aria-hidden="true" className="left-rail__icon-svg">
                <path
                  d="M12 8.8a3.2 3.2 0 1 0 0 6.4 3.2 3.2 0 0 0 0-6.4zm8 3.2l-1.8-.7a6.8 6.8 0 0 0-.4-1l.8-1.8-2.1-2.1-1.8.8a6.8 6.8 0 0 0-1-.4L13 4h-2l-.7 1.8a6.8 6.8 0 0 0-1 .4l-1.8-.8-2.1 2.1.8 1.8a6.8 6.8 0 0 0-.4 1L4 12v2l1.8.7a6.8 6.8 0 0 0 .4 1l-.8 1.8 2.1 2.1 1.8-.8a6.8 6.8 0 0 0 1 .4L11 20h2l.7-1.8a6.8 6.8 0 0 0 1-.4l1.8.8 2.1-2.1-.8-1.8a6.8 6.8 0 0 0 .4-1L20 14v-2z"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.6"
                />
              </svg>
            </span>
          </button>
        ))}
      </nav>
    </aside>
  )
}
