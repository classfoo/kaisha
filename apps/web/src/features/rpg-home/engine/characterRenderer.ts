import { Direction } from './direction'

export type CharacterKind = 'president' | 'employee'

export type CharacterPose = 'standing' | 'sitting'

type Palette = {
  skin: string
  hair: string
  top: string
  bottom: string
  shoe: string
  accent: string
}

const PRESIDENT_PALETTE: Palette = {
  skin: '#f2c9a0',
  hair: '#3d2f28',
  top: '#4f86e8',
  bottom: '#2f4678',
  shoe: '#1c1c1c',
  accent: '#ffd56a',
}

const EMPLOYEE_PALETTES: Palette[] = [
  {
    skin: '#f2c9a0',
    hair: '#2f241f',
    top: '#58c48c',
    bottom: '#3a4a52',
    shoe: '#1c1c1c',
    accent: '#9be7c4',
  },
  {
    skin: '#e8b992',
    hair: '#4a3428',
    top: '#6ec9a8',
    bottom: '#33424a',
    shoe: '#1c1c1c',
    accent: '#b8f0d8',
  },
  {
    skin: '#f0c49a',
    hair: '#262018',
    top: '#7fdca5',
    bottom: '#2f3d44',
    shoe: '#1c1c1c',
    accent: '#c6f5de',
  },
]

export type DrawCharacterOptions = {
  ctx: CanvasRenderingContext2D
  centerX: number
  centerY: number
  tileSize: number
  kind: CharacterKind
  direction: Direction
  isWalking: boolean
  walkFrame: number
  idleFrame: number
  variant?: number
  pose?: CharacterPose
  sitFrame?: number
}

function paletteFor(kind: CharacterKind, variant = 0): Palette {
  if (kind === 'president') return PRESIDENT_PALETTE
  return EMPLOYEE_PALETTES[variant % EMPLOYEE_PALETTES.length]
}

function px(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, color: string) {
  ctx.fillStyle = color
  ctx.fillRect(Math.round(x), Math.round(y), w, h)
}

function drawShadow(ctx: CanvasRenderingContext2D, centerX: number, feetY: number, tileSize: number) {
  ctx.fillStyle = 'rgba(0,0,0,0.28)'
  ctx.beginPath()
  ctx.ellipse(centerX, feetY + tileSize * 0.08, tileSize * 0.22, tileSize * 0.08, 0, 0, Math.PI * 2)
  ctx.fill()
}

function legOffset(walkFrame: number, isWalking: boolean, side: 'left' | 'right'): number {
  if (!isWalking) return 0
  const stride = [0, 2, 0, -2][walkFrame % 4]
  return side === 'left' ? stride : -stride
}

function idleBob(idleFrame: number, isWalking: boolean): number {
  if (isWalking) return 0
  return idleFrame === 0 ? 0 : -1
}

function drawFront(
  ctx: CanvasRenderingContext2D,
  centerX: number,
  baseY: number,
  scale: number,
  palette: Palette,
  kind: CharacterKind,
  isWalking: boolean,
  walkFrame: number,
) {
  const s = scale
  const leftLeg = legOffset(walkFrame, isWalking, 'left') * s
  const rightLeg = legOffset(walkFrame, isWalking, 'right') * s

  px(ctx, centerX - 4 * s, baseY - 3 * s + leftLeg, 3 * s, 5 * s, palette.bottom)
  px(ctx, centerX + 1 * s, baseY - 3 * s + rightLeg, 3 * s, 5 * s, palette.bottom)
  px(ctx, centerX - 4 * s, baseY + 1 * s + leftLeg, 3 * s, 2 * s, palette.shoe)
  px(ctx, centerX + 1 * s, baseY + 1 * s + rightLeg, 3 * s, 2 * s, palette.shoe)

  px(ctx, centerX - 5 * s, baseY - 11 * s, 10 * s, 8 * s, palette.top)
  if (kind === 'president') {
    px(ctx, centerX - 4 * s, baseY - 10 * s, 8 * s, 1 * s, palette.accent)
    px(ctx, centerX - 1 * s, baseY - 9 * s, 2 * s, 4 * s, '#ffffff')
  }

  px(ctx, centerX - 4 * s, baseY - 16 * s, 8 * s, 6 * s, palette.skin)
  px(ctx, centerX - 5 * s, baseY - 18 * s, 10 * s, 3 * s, palette.hair)
  px(ctx, centerX - 2 * s, baseY - 14 * s, 1 * s, 1 * s, '#1f1f1f')
  px(ctx, centerX + 1 * s, baseY - 14 * s, 1 * s, 1 * s, '#1f1f1f')
}

function drawBack(
  ctx: CanvasRenderingContext2D,
  centerX: number,
  baseY: number,
  scale: number,
  palette: Palette,
  isWalking: boolean,
  walkFrame: number,
) {
  const s = scale
  const leftLeg = legOffset(walkFrame, isWalking, 'left') * s
  const rightLeg = legOffset(walkFrame, isWalking, 'right') * s

  px(ctx, centerX - 4 * s, baseY - 3 * s + leftLeg, 3 * s, 5 * s, palette.bottom)
  px(ctx, centerX + 1 * s, baseY - 3 * s + rightLeg, 3 * s, 5 * s, palette.bottom)
  px(ctx, centerX - 4 * s, baseY + 1 * s + leftLeg, 3 * s, 2 * s, palette.shoe)
  px(ctx, centerX + 1 * s, baseY + 1 * s + rightLeg, 3 * s, 2 * s, palette.shoe)
  px(ctx, centerX - 5 * s, baseY - 11 * s, 10 * s, 8 * s, palette.top)
  px(ctx, centerX - 5 * s, baseY - 18 * s, 10 * s, 8 * s, palette.hair)
}

function drawSide(
  ctx: CanvasRenderingContext2D,
  centerX: number,
  baseY: number,
  scale: number,
  palette: Palette,
  direction: Direction,
  kind: CharacterKind,
  isWalking: boolean,
  walkFrame: number,
) {
  const s = scale
  const facingRight = direction === Direction.Right
  const stride = legOffset(walkFrame, isWalking, walkFrame % 2 === 0 ? 'left' : 'right') * s
  const bodyX = centerX + (facingRight ? -3 : -5) * s

  px(ctx, bodyX, baseY - 3 * s + stride, 4 * s, 5 * s, palette.bottom)
  px(ctx, bodyX, baseY + 1 * s + stride, 4 * s, 2 * s, palette.shoe)
  px(ctx, bodyX - 1 * s, baseY - 11 * s, 6 * s, 8 * s, palette.top)
  if (kind === 'president') {
    px(ctx, bodyX + (facingRight ? 2 : 0) * s, baseY - 10 * s, 2 * s, 4 * s, palette.accent)
  }
  px(ctx, bodyX, baseY - 16 * s, 5 * s, 6 * s, palette.skin)
  px(ctx, bodyX + (facingRight ? 1 : -1) * s, baseY - 18 * s, 6 * s, 3 * s, palette.hair)
  px(ctx, bodyX + (facingRight ? 3 : 0) * s, baseY - 14 * s, 1 * s, 1 * s, '#1f1f1f')
}

function drawSittingFacingDown(
  ctx: CanvasRenderingContext2D,
  centerX: number,
  baseY: number,
  scale: number,
  palette: Palette,
  kind: CharacterKind,
  sitFrame: number,
  idleFrame: number,
) {
  const s = scale
  const bob = idleFrame === 0 ? 0 : -1
  const seatY = baseY - 2 * s + bob
  const typingOffset = sitFrame % 2 === 0 ? 0 : -1

  px(ctx, centerX - 5 * s, seatY - 2 * s, 10 * s, 4 * s, palette.bottom)

  const leftArmY = seatY - 6 * s + typingOffset
  const rightArmY = seatY - 6 * s + (typingOffset === 0 ? -1 : 0)
  px(ctx, centerX - 8 * s, leftArmY, 3 * s, 2 * s, palette.skin)
  px(ctx, centerX + 5 * s, rightArmY, 3 * s, 2 * s, palette.skin)

  px(ctx, centerX - 5 * s, seatY - 8 * s, 10 * s, 7 * s, palette.top)
  if (kind === 'president') {
    px(ctx, centerX - 4 * s, seatY - 7 * s, 8 * s, 1 * s, palette.accent)
    px(ctx, centerX - 1 * s, seatY - 6 * s, 2 * s, 3 * s, '#ffffff')
  }

  px(ctx, centerX - 4 * s, seatY - 14 * s, 8 * s, 6 * s, palette.skin)
  px(ctx, centerX - 5 * s, seatY - 16 * s, 10 * s, 3 * s, palette.hair)
  px(ctx, centerX - 2 * s, seatY - 12 * s, 1 * s, 1 * s, '#1f1f1f')
  px(ctx, centerX + 1 * s, seatY - 12 * s, 1 * s, 1 * s, '#1f1f1f')
}

export function drawCharacter(options: DrawCharacterOptions) {
  const {
    ctx,
    centerX,
    centerY,
    tileSize,
    kind,
    direction,
    isWalking,
    walkFrame,
    idleFrame,
    variant = 0,
    pose = 'standing',
    sitFrame = 0,
  } = options

  const palette = paletteFor(kind, variant)
  const scale = Math.max(2, Math.floor(tileSize / 16))
  const bob = idleBob(idleFrame, isWalking)
  const sitting = pose === 'sitting'
  const feetY = centerY + tileSize * (sitting ? 0.26 : 0.18) + bob
  const baseY = feetY

  if (!sitting) {
    drawShadow(ctx, centerX, feetY, tileSize)
  }

  if (sitting && direction === Direction.Down) {
    drawSittingFacingDown(ctx, centerX, baseY, scale, palette, kind, sitFrame, idleFrame)
    return
  }

  if (sitting && direction === Direction.Up) {
    drawBack(ctx, centerX, baseY, scale, palette, false, 0)
    return
  }

  switch (direction) {
    case Direction.Down:
      drawFront(ctx, centerX, baseY, scale, palette, kind, isWalking, walkFrame)
      break
    case Direction.Up:
      drawBack(ctx, centerX, baseY, scale, palette, isWalking, walkFrame)
      break
    case Direction.Left:
    case Direction.Right:
      drawSide(ctx, centerX, baseY, scale, palette, direction, kind, isWalking, walkFrame)
      break
  }
}
