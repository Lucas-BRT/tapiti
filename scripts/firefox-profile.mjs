// A fresh, empty Firefox profile for the headless checks.
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

// SCHEME=dark or SCHEME=light makes the pages see that colour scheme (the site follows prefers-color-scheme), whatever the system says.
const SCHEMES = { dark: 0, light: 1 }
const PREFS = process.env.SCHEME in SCHEMES ? [['layout.css.prefers-color-scheme.content-override', SCHEMES[process.env.SCHEME]]] : []

export const freshProfile = (name) => {
  const dir = mkdtempSync(join(tmpdir(), `tapiti-${name}-`))
  writeFileSync(join(dir, 'user.js'), PREFS.map(([key, value]) => `user_pref(${JSON.stringify(key)}, ${value});\n`).join(''))
  return dir
}
