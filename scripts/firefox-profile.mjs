// A fresh, empty Firefox profile for the headless checks.
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const PREFS = []

export const freshProfile = (name) => {
  const dir = mkdtempSync(join(tmpdir(), `tapiti-${name}-`))
  writeFileSync(join(dir, 'user.js'), PREFS.map(([key, value]) => `user_pref(${JSON.stringify(key)}, ${value});\n`).join(''))
  return dir
}
