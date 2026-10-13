import { describe, expect, it } from 'vitest'

import { snapMoveToBubble } from '@/lib/geometry'

describe('edge and alignment snap', () => {
  const bubble = { x: 100, y: 50, width: 200, height: 100 }
  const box = { x: 110, y: 60, width: 20, height: 10, angle_degrees: 0 }
  const snap = (dx: number, dy: number, others: (typeof bubble)[] = [], zoom = 1) =>
    snapMoveToBubble([box], { x: dx, y: dy }, [bubble], { zoom, others })

  it('snaps box edges flush to the bubble edges', () => {
    // left edge 105 -> 100; bottom edge 60 + 85 + 10 = 155 -> 150
    const result = snap(-5, 85)
    expect(result.delta).toEqual({ x: -10, y: 80 })
    expect(result.guide).toEqual({ x: 100, y: 150, bubble })
  })

  it('aligns with other boxes, also outside any bubble', () => {
    const other = { x: 500, y: 400, width: 40, height: 40 }
    const result = snapMoveToBubble([{ ...box, x: 700 }], { x: -203, y: 0 }, [bubble], {
      zoom: 1,
      others: [other],
    })
    expect(result.delta.x).toBe(-200) // left 497 -> 500
    expect(result.guide).toEqual({ x: 500 })
  })

  it('ignores candidates beyond 8 screen px, at any zoom', () => {
    const far = { x: 145, y: 300, width: 0, height: 0 }
    expect(snap(0, 0, [far]).guide).toBeNull() // 15 from the box right edge
    const near = { x: 117, y: 300, width: 0, height: 0 } // box center 120, 3 away
    expect(snap(0, 0, [near], 1).guide?.x).toBe(117)
    expect(snap(0, 0, [near], 4).guide).toBeNull() // tolerance 2 page px
  })

  it('prefers the bubble center on ties', () => {
    // moved center x = 203: bubble center 200 and the other line 206 are both 3 away
    const result = snap(83, 0, [{ x: 206, y: 300, width: 0, height: 0 }])
    expect(result.delta.x).toBe(80)
    expect(result.guide?.x).toBe(200)
  })
})
