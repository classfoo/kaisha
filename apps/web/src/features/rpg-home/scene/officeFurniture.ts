import { Direction } from '../engine/direction'
import { GridPoint, RpgFurniture } from '../types'
import { WORKSTATION_ANCHORS } from './workstationAnchors'

function workstationForAnchor(anchor: GridPoint, index: number): RpgFurniture {
  return {
    id: `workstation-${index}`,
    kind: 'workstation',
    x: anchor.x,
    y: anchor.y + 1,
    width: 2,
    height: 1,
    facing: Direction.Up,
  }
}

function chairForAnchor(anchor: GridPoint, index: number): RpgFurniture {
  return {
    id: `chair-${index}`,
    kind: 'office_chair',
    x: anchor.x,
    y: anchor.y,
    width: 1,
    height: 1,
    facing: Direction.Down,
  }
}

export const BOSS_CHAIR: GridPoint = { x: 3, y: 3 }

export function buildOfficeFurniture(): RpgFurniture[] {
  const workstations = WORKSTATION_ANCHORS.flatMap((anchor, index) => [
    workstationForAnchor(anchor, index),
    chairForAnchor(anchor, index),
  ])

  const bossOffice: RpgFurniture[] = [
    {
      id: 'boss-desk',
      kind: 'boss_desk',
      x: 1,
      y: 4,
      width: 4,
      height: 1,
      facing: Direction.Up,
    },
    {
      id: 'boss-chair',
      kind: 'office_chair',
      x: BOSS_CHAIR.x,
      y: BOSS_CHAIR.y,
      width: 1,
      height: 1,
      facing: Direction.Down,
    },
    {
      id: 'boss-sofa',
      kind: 'sofa',
      x: 5,
      y: 3,
      width: 2,
      height: 2,
      facing: Direction.Left,
    },
  ]

  return [...workstations, ...bossOffice]
}

export function furnitureBlockedTiles(furniture: RpgFurniture[]): Array<{ x: number; y: number }> {
  const blocked: Array<{ x: number; y: number }> = []
  for (const item of furniture) {
    if (item.kind === 'office_chair') {
      continue
    }
    for (let dx = 0; dx < item.width; dx += 1) {
      for (let dy = 0; dy < item.height; dy += 1) {
        blocked.push({ x: item.x + dx, y: item.y + dy })
      }
    }
  }
  return blocked
}
