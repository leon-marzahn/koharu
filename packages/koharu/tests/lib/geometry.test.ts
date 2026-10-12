import { describe, expect, it } from 'vitest'

import { resizeFrame, rotateFrame, snapMoveToBubble } from '@/lib/geometry'
import type { Frame } from '@koharu/bridge/protocol'

const frame: Frame = { x: 10, y: 20, width: 100, height: 50, angle_degrees: 0 }

describe('selection control geometry', () => {
  it('resizes from a handle while keeping the opposite edge fixed', () => {
    expect(resizeFrame(frame, 'e', { x: 140, y: 45 }, 1)).toEqual({
      ...frame,
      width: 130,
    })
    expect(resizeFrame(frame, 'nw', { x: 0, y: 10 }, 1)).toEqual({
      ...frame,
      x: 0,
      y: 10,
      width: 110,
      height: 60,
    })
  })

  it('resizes in the frame local axes when it is rotated', () => {
    const result = resizeFrame({ ...frame, angle_degrees: 90 }, 'e', { x: 60, y: 125 }, 1)
    expect(result.x).toBeCloseTo(-5)
    expect(result.y).toBeCloseTo(35)
    expect(result.width).toBeCloseTo(130)
    expect(result.height).toBe(50)
  })

  it('rotates around the frame center', () => {
    expect(rotateFrame(frame, { x: 60, y: -5 }, { x: 110, y: 45 })).toEqual({
      ...frame,
      angle_degrees: 90,
    })
  })
})

describe('center snap', () => {
  // Bubble center is (200, 100).
  const bubble = { x: 100, y: 50, width: 200, height: 100 }
  const box = { x: 170, y: 80, width: 20, height: 10, angle_degrees: 0 } // center (180, 85)
  const move = (dx: number, dy: number, options = {}) =>
    snapMoveToBubble([box], { x: dx, y: dy }, [bubble], { zoom: 1, ...options })

  it('snaps each axis independently within 8 screen px', () => {
    // moved center (205, 90): x is 5 away, y is 10 away
    const result = move(25, 5)
    expect(result.delta).toEqual({ x: 20, y: 5 })
    expect(result.guide).toEqual({ x: 200, bubble })
  })

  it('snaps both axes and reports both guides', () => {
    const result = move(22, 18)
    expect(result.delta).toEqual({ x: 20, y: 15 })
    expect(result.guide).toEqual({ x: 200, y: 100, bubble })
  })

  it('does not snap beyond the threshold', () => {
    const result = move(29, 29)
    expect(result.delta).toEqual({ x: 29, y: 29 })
    expect(result.guide).toBeNull()
  })

  it('converts the 8 screen px tolerance to page px by zoom', () => {
    // moved center x = 207: 7 page px away
    expect(move(27, 0, { zoom: 1 }).guide).not.toBeNull()
    expect(move(27, 0, { zoom: 2 }).guide).toBeNull() // tolerance 4 page px
    expect(move(27, 0, { zoom: 0.5 }).guide).not.toBeNull() // tolerance 16 page px
  })

  it('never snaps when the Cmd override is held', () => {
    const result = move(22, 18, { disabled: true })
    expect(result.delta).toEqual({ x: 22, y: 18 })
    expect(result.guide).toBeNull()
  })

  it('never snaps a box whose center is outside every bubble', () => {
    const outside = { ...box, x: 600 }
    const result = snapMoveToBubble([outside], { x: 0, y: 0 }, [bubble], { zoom: 1 })
    expect(result.guide).toBeNull()
  })
})
