import { EmployeeDirectoryRecord } from '../../components/EmployeeList'
import { Direction } from './engine/direction'

export type RpgZoneType = 'desk' | 'meeting' | 'lounge' | 'reception'

export type RpgFurnitureKind = 'workstation' | 'boss_desk' | 'sofa' | 'office_chair'

export type RpgFurniture = {
  id: string
  kind: RpgFurnitureKind
  x: number
  y: number
  width: number
  height: number
  facing?: Direction
}

export type GridPoint = { x: number; y: number }

export type RpgZone = {
  id: string
  type: RpgZoneType
  labelKey: string
  x: number
  y: number
  width: number
  height: number
}

export type RpgActorKind = 'president' | 'employee'

export type RpgActor = {
  id: string
  kind: RpgActorKind
  name: string
  department?: string
  role?: string
  position: GridPoint
}

export type RpgOfficeScene = {
  width: number
  height: number
  tileSize: number
  zones: RpgZone[]
  walls: GridPoint[]
  furniture: RpgFurniture[]
}

export type RpgHomeSnapshot = {
  scene: RpgOfficeScene
  player: RpgActor
  employees: RpgActor[]
}

export type EmployeeRecord = EmployeeDirectoryRecord
