import { describe, expect, test } from 'bun:test'
import { composite, hexColor, parseColor } from './mermaid-diagram'

describe('mermaid color resolution', () => {
  test('hex colors parse including short forms', () => {
    expect(parseColor('#1a1a1a')).toEqual({ r: 26, g: 26, b: 26, a: 1 })
    expect(parseColor('#abc')).toEqual({ r: 170, g: 187, b: 204, a: 1 })
    const alpha = parseColor('#ff000080')
    expect(alpha?.r).toBe(255)
    expect(alpha?.g).toBe(0)
    expect(alpha?.b).toBe(0)
    expect(alpha?.a).toBeCloseTo(128 / 255)
    expect(parseColor('nope')).toBeNull()
  })

  test('rgb() with modern space syntax and alpha parses', () => {
    expect(parseColor('rgb(28 31 37 / 8%)')).toEqual({ r: 28, g: 31, b: 37, a: 0.08 })
    expect(parseColor('rgb(255, 0, 0)')).toEqual({ r: 255, g: 0, b: 0, a: 1 })
  })

  test('hsl() converts to rgb', () => {
    expect(parseColor('hsl(120 50% 50%)')).toEqual({ r: 64, g: 191, b: 64, a: 1 })
  })

  test('translucent colors composite to solid over a surface', () => {
    // --border is rgb(... / 8%): a subtle 8% black border over white.
    const border = parseColor('rgb(28 31 37 / 8%)')!
    const white = { r: 255, g: 255, b: 255, a: 1 }
    expect(hexColor(composite(border, white))).toBe('#ededee')
  })

  test('opaque colors survive compositing unchanged', () => {
    const card = parseColor('#ffffff')!
    const surface = parseColor('#f6f5f6')!
    expect(hexColor(composite(card, surface))).toBe('#ffffff')
  })
})