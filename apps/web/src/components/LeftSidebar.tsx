import React from 'react'

export type NavMenu = 'workspace' | 'explorer' | 'search' | 'settings'

type LeftSidebarProps = {
  activeNav: NavMenu
  navItems: { id: NavMenu; labelKey: string; icon: string }[]
  t: (key: string) => string
  onMenuClick: (menu: NavMenu) => void
}

export function LeftSidebar({ activeNav, navItems, t, onMenuClick }: LeftSidebarProps) {
  return (
    <aside className="left-rail">
      <div className="left-rail__top-space" />
      <nav className="left-rail__menu">
        {navItems.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`left-rail__btn ${activeNav === item.id ? 'left-rail__btn--active' : ''}`}
            aria-label={t(item.labelKey)}
            title={t(item.labelKey)}
            onClick={() => onMenuClick(item.id)}
          >
            {item.icon}
          </button>
        ))}
      </nav>
    </aside>
  )
}
