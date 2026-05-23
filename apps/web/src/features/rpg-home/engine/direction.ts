export enum Direction {
  Up = 'up',
  Down = 'down',
  Left = 'left',
  Right = 'right',
}

export function directionDelta(direction: Direction): { dx: number; dy: number } {
  switch (direction) {
    case Direction.Up:
      return { dx: 0, dy: -1 }
    case Direction.Down:
      return { dx: 0, dy: 1 }
    case Direction.Left:
      return { dx: -1, dy: 0 }
    case Direction.Right:
      return { dx: 1, dy: 0 }
  }
}
