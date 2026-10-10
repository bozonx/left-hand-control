import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { parseLayoutYaml } from '~/utils/layoutPresets'

// The Rust core parses the same fixture in
// crates/lhc-core/src/profile/layout_file.rs; both must match the
// expected JSON so the two shells read layout files identically.
describe('layout YAML parity with lhc-core', () => {
  it('parses the shared fixture into the expected preset', () => {
    const text = readFileSync('tests/fixtures/layout-parity.yaml', 'utf8')
    const expected = JSON.parse(
      readFileSync('tests/fixtures/layout-parity.expected.json', 'utf8'),
    )
    const preset = JSON.parse(JSON.stringify(parseLayoutYaml(text)))
    for (const macro of preset.macros) {
      for (const step of macro.steps) {
        if (step.id !== 's3') step.id = '<generated>'
      }
    }
    expect(preset).toEqual(expected)
  })
})
