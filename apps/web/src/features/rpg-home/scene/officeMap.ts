import { RpgOfficeScene } from '../types'
import { buildOfficeFurniture } from './officeFurniture'

const wall = (x: number, y: number) => ({ x, y })

export const officeScene: RpgOfficeScene = {
  width: 28,
  height: 18,
  tileSize: 32,
  zones: [
    { id: 'reception', type: 'reception', labelKey: 'ui.rpgHome.zone.reception', x: 1, y: 1, width: 7, height: 5 },
    { id: 'desk', type: 'desk', labelKey: 'ui.rpgHome.zone.desk', x: 9, y: 1, width: 18, height: 9 },
    { id: 'meeting', type: 'meeting', labelKey: 'ui.rpgHome.zone.meeting', x: 1, y: 7, width: 11, height: 10 },
    { id: 'lounge', type: 'lounge', labelKey: 'ui.rpgHome.zone.lounge', x: 13, y: 11, width: 14, height: 6 },
  ],
  walls: [
    ...Array.from({ length: 28 }, (_, x) => wall(x, 0)),
    ...Array.from({ length: 28 }, (_, x) => wall(x, 17)),
    ...Array.from({ length: 18 }, (_, y) => wall(0, y)),
    ...Array.from({ length: 18 }, (_, y) => wall(27, y)),
  ],
  furniture: buildOfficeFurniture(),
}
