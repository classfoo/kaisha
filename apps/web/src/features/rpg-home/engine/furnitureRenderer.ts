import { Direction } from './direction'
import { RpgFurniture } from '../types'

function px(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, color: string) {
  ctx.fillStyle = color
  ctx.fillRect(Math.round(x), Math.round(y), w, h)
}

function tileOrigin(x: number, y: number, tileSize: number) {
  return { ox: x * tileSize, oy: y * tileSize }
}

function drawWorkstation(ctx: CanvasRenderingContext2D, item: RpgFurniture, tileSize: number) {
  const { ox, oy } = tileOrigin(item.x, item.y, tileSize)
  const w = item.width * tileSize
  const h = item.height * tileSize
  const facesChair = item.facing === Direction.Up

  px(ctx, ox + 1, oy + h * 0.08, w - 2, h * 0.84, '#5a4638')
  px(ctx, ox + 2, oy + h * 0.1, w - 4, h * 0.06, '#7a5c48')
  px(ctx, ox + 3, oy + h * 0.72, w * 0.18, h * 0.22, '#4a5568')
  px(ctx, ox + w - w * 0.18 - 3, oy + h * 0.72, w * 0.18, h * 0.22, '#4a5568')

  const monitorW = Math.round(w * 0.34)
  const monitorH = Math.round(monitorW * (9 / 16))
  const monitorX = ox + Math.round((w - monitorW) / 2)
  const monitorY = facesChair ? oy + 4 : oy + h - monitorH - 8

  px(ctx, monitorX - 1, monitorY - 1, monitorW + 2, monitorH + 2, '#1a1f28')
  px(ctx, monitorX, monitorY, monitorW, monitorH, '#252b36')
  px(ctx, monitorX + 2, monitorY + 2, monitorW - 4, monitorH - 4, '#1e2838')
  px(ctx, monitorX + 3, monitorY + 3, monitorW - 6, monitorH - 6, '#3a7cc4')

  const standW = Math.max(4, Math.round(monitorW * 0.14))
  const standX = monitorX + Math.round((monitorW - standW) / 2)
  const standY = facesChair ? monitorY + monitorH : monitorY - 5
  px(ctx, standX + standW * 0.35, standY, standW * 0.3, 5, '#2a3038')
  px(ctx, standX, standY + (facesChair ? 4 : -3), standW, 3, '#343b46')

  const kbW = Math.round(w * 0.42)
  const kbH = Math.max(3, Math.round(h * 0.07))
  const kbX = ox + Math.round((w - kbW) / 2)
  const kbY = facesChair ? oy + h * 0.52 : oy + h * 0.28
  px(ctx, kbX, kbY, kbW, kbH, '#2f3640')
  px(ctx, kbX + 2, kbY + 1, kbW - 4, kbH - 2, '#3d4654')
}

function drawOfficeChair(ctx: CanvasRenderingContext2D, item: RpgFurniture, tileSize: number) {
  const { ox, oy } = tileOrigin(item.x, item.y, tileSize)
  const cx = ox + tileSize / 2
  const facingDown = item.facing === Direction.Down
  const seatY = oy + tileSize * 0.56

  ctx.fillStyle = 'rgba(0,0,0,0.22)'
  ctx.beginPath()
  ctx.ellipse(cx, oy + tileSize * 0.82, tileSize * 0.18, tileSize * 0.06, 0, 0, Math.PI * 2)
  ctx.fill()

  px(ctx, cx - 3, seatY + 8, 6, 5, '#2a3038')
  px(ctx, cx - 8, seatY + 11, 16, 3, '#343b46')

  px(ctx, cx - 9, seatY, 18, 6, '#4a5568')
  px(ctx, cx - 8, seatY + 1, 16, 4, '#5c6575')

  if (facingDown) {
    px(ctx, cx - 7, seatY - 14, 14, 14, '#3d4654')
    px(ctx, cx - 6, seatY - 13, 12, 12, '#4a5568')
    px(ctx, cx - 5, seatY - 12, 10, 9, '#5c6575')
  } else {
    px(ctx, cx - 7, seatY + 2, 14, 10, '#3d4654')
  }

  px(ctx, cx - 10, seatY - 4, 4, 8, '#4a5568')
  px(ctx, cx + 6, seatY - 4, 4, 8, '#4a5568')
}

function drawBossDesk(ctx: CanvasRenderingContext2D, item: RpgFurniture, tileSize: number) {
  const { ox, oy } = tileOrigin(item.x, item.y, tileSize)
  const w = item.width * tileSize
  const h = item.height * tileSize
  const facesChair = item.facing === Direction.Up

  px(ctx, ox + 2, oy + 4, w - 4, h - 6, '#5c4030')
  px(ctx, ox + 4, oy + 6, w - 8, h - 10, '#7a5840')
  px(ctx, ox + 6, oy + 8, w - 12, 4, '#8f6848')

  const monitorW = Math.round(w * 0.2)
  const monitorH = Math.round(monitorW * (9 / 16))
  const monitorX = ox + Math.round(w * 0.22)
  const monitorY = facesChair ? oy + 5 : oy + h - monitorH - 8
  px(ctx, monitorX - 1, monitorY - 1, monitorW + 2, monitorH + 2, '#1a1f28')
  px(ctx, monitorX + 2, monitorY + 2, monitorW - 4, monitorH - 4, '#3a7cc4')

  px(ctx, ox + w * 0.58, oy + (facesChair ? h * 0.22 : h * 0.42), w * 0.22, h * 0.28, '#d8cbb8')
  px(ctx, ox + w * 0.6, oy + (facesChair ? h * 0.24 : h * 0.44), w * 0.18, h * 0.22, '#f5efe4')

  px(ctx, ox + 8, oy + h - 7, 8, 6, '#3f3028')
  px(ctx, ox + w - 16, oy + h - 7, 8, 6, '#3f3028')
}

function drawSofa(ctx: CanvasRenderingContext2D, item: RpgFurniture, tileSize: number) {
  const { ox, oy } = tileOrigin(item.x, item.y, tileSize)
  const w = item.width * tileSize
  const h = item.height * tileSize
  const facingLeft = item.facing === Direction.Left

  const backX = facingLeft ? ox + w - 10 : ox + 2
  px(ctx, backX, oy + 4, 8, h - 8, '#4a5568')
  px(ctx, ox + 4, oy + 6, w - 8, h - 12, '#6b7280')
  px(ctx, ox + 6, oy + 10, w - 12, h - 18, '#8b95a5')

  px(ctx, ox + 2, oy + 2, w - 4, 8, '#5c6575')
  px(ctx, ox + 4, oy + h - 8, w - 8, 6, '#4a5568')

  px(ctx, ox + 8, oy + 12, 10, 8, '#9aa3b2')
  px(ctx, ox + w - 18, oy + 12, 10, 8, '#9aa3b2')
}

export function drawFurniture(ctx: CanvasRenderingContext2D, item: RpgFurniture, tileSize: number) {
  switch (item.kind) {
    case 'workstation':
      drawWorkstation(ctx, item, tileSize)
      break
    case 'office_chair':
      drawOfficeChair(ctx, item, tileSize)
      break
    case 'boss_desk':
      drawBossDesk(ctx, item, tileSize)
      break
    case 'sofa':
      drawSofa(ctx, item, tileSize)
      break
  }
}

export function drawAllFurniture(
  ctx: CanvasRenderingContext2D,
  furniture: RpgFurniture[],
  tileSize: number,
) {
  const desks = furniture.filter((item) => item.kind !== 'office_chair')
  const chairs = furniture.filter((item) => item.kind === 'office_chair')
  desks.forEach((item) => drawFurniture(ctx, item, tileSize))
  chairs.forEach((item) => drawFurniture(ctx, item, tileSize))
}

export function drawWorkstationDesksOnly(
  ctx: CanvasRenderingContext2D,
  furniture: RpgFurniture[],
  tileSize: number,
) {
  furniture
    .filter((item) => item.kind === 'workstation' || item.kind === 'boss_desk' || item.kind === 'sofa')
    .forEach((item) => drawFurniture(ctx, item, tileSize))
}

export function drawWorkstationChairsOnly(
  ctx: CanvasRenderingContext2D,
  furniture: RpgFurniture[],
  tileSize: number,
) {
  furniture.filter((item) => item.kind === 'office_chair').forEach((item) => drawFurniture(ctx, item, tileSize))
}
