import React from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'

type MacWindowControlsProps = {
  locale?: string
  t: (key: string) => string
}

// 检测是否在 Tauri 环境中运行
const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

export const MacWindowControls: React.FC<MacWindowControlsProps> = ({ t }) => {
  // 只在 Tauri 环境中获取窗口实例
  const appWindow = isTauri ? getCurrentWindow() : null

  const handleWindowControl = async (action: 'close' | 'minimize' | 'maximize') => {
    if (!appWindow) return
    if (action === 'close') {
      await appWindow.close()
      return
    }
    if (action === 'minimize') {
      await appWindow.minimize()
      return
    }
    const maximized = await appWindow.isMaximized()
    if (maximized) {
      await appWindow.unmaximize()
    } else {
      await appWindow.maximize()
    }
  }

  // 在浏览器模式下不渲染窗口控制按钮
  if (!isTauri) return null

  return (
    <div className="mac-controls" onMouseDown={(e) => e.stopPropagation()}>
      <button
        className="mac-control mac-control--close"
        onClick={() => void handleWindowControl('close')}
        aria-label={t('ui.window.close')}
        title={t('ui.window.close')}
      />
      <button
        className="mac-control mac-control--minimize"
        onClick={() => void handleWindowControl('minimize')}
        aria-label={t('ui.window.minimize')}
        title={t('ui.window.minimize')}
      />
      <button
        className="mac-control mac-control--maximize"
        onClick={() => void handleWindowControl('maximize')}
        aria-label={t('ui.window.maximize')}
        title={t('ui.window.maximize')}
      />
    </div>
  )
}