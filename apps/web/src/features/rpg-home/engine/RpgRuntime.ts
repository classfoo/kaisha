import { BOSS_CHAIR, furnitureBlockedTiles } from '../scene/officeFurniture'
import { RpgHomeSnapshot } from '../types'
import { drawCharacter } from './characterRenderer'
import { Direction } from './direction'
import { drawWorkstationChairsOnly, drawWorkstationDesksOnly } from './furnitureRenderer'
import { createMotionState, MotionState, updatePlayerMotion } from './playerMotion'

type RuntimeOptions = {
  container: HTMLElement
  snapshot: RpgHomeSnapshot
  onInteractEmployee: (employeeActorId: string | null) => void
}

const WALL_COLOR = '#2b3242'
const GRID_COLOR = '#202736'

type EmployeeVisual = {
  id: string
  pixelX: number
  pixelY: number
  direction: Direction
  variant: number
  idleFrame: number
  idleTimerMs: number
  sitFrame: number
  sitTimerMs: number
}

export class RpgRuntime {
  private readonly container: HTMLElement
  private readonly snapshot: RpgHomeSnapshot
  private readonly onInteractEmployee: (employeeActorId: string | null) => void
  private readonly canvas: HTMLCanvasElement
  private readonly context: CanvasRenderingContext2D
  private readonly pressed = new Set<string>()
  private animationId: number | null = null
  private readonly keydownListener: (event: KeyboardEvent) => void
  private readonly keyupListener: (event: KeyboardEvent) => void
  private readonly blockedTiles: Array<{ x: number; y: number }>
  private playerMotion: MotionState
  private employeeVisuals: EmployeeVisual[]
  private lastFrameTime = 0

  constructor(options: RuntimeOptions) {
    this.container = options.container
    this.snapshot = options.snapshot
    this.onInteractEmployee = options.onInteractEmployee
    this.canvas = document.createElement('canvas')
    this.canvas.width = this.snapshot.scene.width * this.snapshot.scene.tileSize
    this.canvas.height = this.snapshot.scene.height * this.snapshot.scene.tileSize
    this.canvas.className = 'rpg-home__canvas'
    this.context = this.canvas.getContext('2d') as CanvasRenderingContext2D
    this.keydownListener = (event) => this.handleKeyDown(event)
    this.keyupListener = (event) => this.handleKeyUp(event)
    this.blockedTiles = [
      ...this.snapshot.scene.walls,
      ...furnitureBlockedTiles(this.snapshot.scene.furniture),
    ]
    const { tileSize } = this.snapshot.scene
    const start = this.snapshot.player.position
    this.playerMotion = createMotionState(start.x, start.y, tileSize)
    this.employeeVisuals = this.snapshot.employees.map((employee, index) => ({
      id: employee.id,
      pixelX: employee.position.x * tileSize + tileSize / 2,
      pixelY: employee.position.y * tileSize + tileSize / 2,
      direction: Direction.Down,
      variant: index,
      idleFrame: index % 2,
      idleTimerMs: index * 120,
      sitFrame: 0,
      sitTimerMs: index * 80,
    }))
  }

  mount() {
    this.container.innerHTML = ''
    this.container.appendChild(this.canvas)
    window.addEventListener('keydown', this.keydownListener)
    window.addEventListener('keyup', this.keyupListener)
    this.lastFrameTime = performance.now()
    this.loop(this.lastFrameTime)
  }

  destroy() {
    if (this.animationId !== null) {
      window.cancelAnimationFrame(this.animationId)
    }
    window.removeEventListener('keydown', this.keydownListener)
    window.removeEventListener('keyup', this.keyupListener)
    this.container.innerHTML = ''
  }

  private handleKeyDown(event: KeyboardEvent) {
    this.pressed.add(event.key.toLowerCase())
    if (event.key.toLowerCase() === 'e') {
      this.tryInteract()
    }
  }

  private handleKeyUp(event: KeyboardEvent) {
    this.pressed.delete(event.key.toLowerCase())
  }

  private loop = (time: number) => {
    const deltaMs = Math.min(time - this.lastFrameTime, 48)
    this.lastFrameTime = time
    this.update(deltaMs)
    this.draw()
    this.animationId = window.requestAnimationFrame(this.loop)
  }

  private directionFromInput(): Direction | null {
    if (this.pressed.has('arrowup') || this.pressed.has('w')) return Direction.Up
    if (this.pressed.has('arrowdown') || this.pressed.has('s')) return Direction.Down
    if (this.pressed.has('arrowleft') || this.pressed.has('a')) return Direction.Left
    if (this.pressed.has('arrowright') || this.pressed.has('d')) return Direction.Right
    return null
  }

  private isBlocked(x: number, y: number): boolean {
    return this.blockedTiles.some((tile) => tile.x === x && tile.y === y)
  }

  private update(deltaMs: number) {
    const { tileSize } = this.snapshot.scene
    this.playerMotion = updatePlayerMotion(
      this.playerMotion,
      this.directionFromInput(),
      (x, y) => !this.isBlocked(x, y),
      tileSize,
      deltaMs,
    )

    this.employeeVisuals = this.employeeVisuals.map((employee) => {
      const next = {
        ...employee,
        idleTimerMs: employee.idleTimerMs + deltaMs,
        sitTimerMs: employee.sitTimerMs + deltaMs,
      }
      if (next.idleTimerMs >= 700) {
        next.idleFrame = (next.idleFrame + 1) % 2
        next.idleTimerMs = 0
      }
      if (next.sitTimerMs >= 320) {
        next.sitFrame = (next.sitFrame + 1) % 4
        next.sitTimerMs = 0
      }
      return next
    })
  }

  private tryInteract() {
    const { gridX, gridY } = this.playerMotion
    const target = this.snapshot.employees.find(
      (employee) =>
        Math.abs(employee.position.x - gridX) <= 1 && Math.abs(employee.position.y - gridY) <= 1,
    )
    this.onInteractEmployee(target?.id ?? null)
  }

  private draw() {
    const { tileSize, width, height } = this.snapshot.scene
    this.context.clearRect(0, 0, this.canvas.width, this.canvas.height)
    this.context.fillStyle = '#151c29'
    this.context.fillRect(0, 0, this.canvas.width, this.canvas.height)

    this.context.strokeStyle = GRID_COLOR
    this.context.lineWidth = 1
    for (let x = 0; x <= width; x += 1) {
      this.context.beginPath()
      this.context.moveTo(x * tileSize, 0)
      this.context.lineTo(x * tileSize, height * tileSize)
      this.context.stroke()
    }
    for (let y = 0; y <= height; y += 1) {
      this.context.beginPath()
      this.context.moveTo(0, y * tileSize)
      this.context.lineTo(width * tileSize, y * tileSize)
      this.context.stroke()
    }

    this.snapshot.scene.zones.forEach((zone) => {
      this.context.fillStyle =
        zone.type === 'reception'
          ? '#2c4868'
          : zone.type === 'desk'
            ? '#355945'
            : zone.type === 'meeting'
              ? '#4d3f68'
              : '#6a5a3b'
      this.context.globalAlpha = 0.42
      this.context.fillRect(zone.x * tileSize, zone.y * tileSize, zone.width * tileSize, zone.height * tileSize)
      this.context.globalAlpha = 1
    })

    this.context.fillStyle = WALL_COLOR
    this.snapshot.scene.walls.forEach((wall) => {
      this.context.fillRect(wall.x * tileSize, wall.y * tileSize, tileSize, tileSize)
    })

    drawWorkstationDesksOnly(this.context, this.snapshot.scene.furniture, tileSize)
    drawWorkstationChairsOnly(this.context, this.snapshot.scene.furniture, tileSize)

    this.employeeVisuals.forEach((employee) => {
      drawCharacter({
        ctx: this.context,
        centerX: employee.pixelX,
        centerY: employee.pixelY,
        tileSize,
        kind: 'employee',
        direction: employee.direction,
        isWalking: false,
        walkFrame: 0,
        idleFrame: employee.idleFrame,
        variant: employee.variant,
        pose: 'sitting',
        sitFrame: employee.sitFrame,
      })
    })

    drawCharacter({
      ctx: this.context,
      centerX: this.playerMotion.pixelX,
      centerY: this.playerMotion.pixelY,
      tileSize,
      kind: 'president',
      direction:
        this.isPresidentSitting() ? Direction.Down : this.playerMotion.facing,
      isWalking: this.playerMotion.isMoving,
      walkFrame: this.playerMotion.walkFrame,
      idleFrame: this.playerMotion.idleFrame,
      pose: this.isPresidentSitting() ? 'sitting' : 'standing',
      sitFrame: this.playerMotion.idleFrame,
    })
  }

  private isPresidentSitting(): boolean {
    const { gridX, gridY, isMoving } = this.playerMotion
    return !isMoving && gridX === BOSS_CHAIR.x && gridY === BOSS_CHAIR.y
  }
}
