// Smoke test of the site in headless Firefox, over WebDriver BiDi (Node 22+ and `firefox`; start the site first).
//   node scripts/site-smoke.mjs [base-url]        default: http://127.0.0.1:3000
// Checks the landing page and the 404, and that the docs load no wasm. With AXE_JS set, also runs axe-core.
import { spawn } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { freshProfile } from './firefox-profile.mjs'

const base = process.argv[2] ?? 'http://127.0.0.1:3000'
const port = 9336
const profile = freshProfile('site')
const firefox = spawn('firefox', ['--headless', '--no-remote', '--profile', profile, `--remote-debugging-port=${port}`], { stdio: 'ignore' })
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
let ws
for (let i = 0; i < 60 && !ws; i++) {
  try {
    ws = await new Promise((resolve, reject) => { const s = new WebSocket(`ws://127.0.0.1:${port}/session`); s.onopen = () => resolve(s); s.onerror = reject })
  } catch { await sleep(500) }
}
if (!ws) { firefox.kill(); console.error('could not reach Firefox'); process.exit(2) }
let id = 0
const waiting = new Map()
const errors = []
ws.onmessage = ({ data }) => {
  const m = JSON.parse(data)
  if (m.id && waiting.has(m.id)) waiting.get(m.id)(m)
  else if (m.method === 'log.entryAdded' && m.params.level === 'error') errors.push(m.params.text)
}
const send = (method, params = {}) => new Promise((resolve) => { const n = ++id; waiting.set(n, resolve); ws.send(JSON.stringify({ id: n, method, params })) })
await send('session.new', { capabilities: {} })
await send('session.subscribe', { events: ['log.entryAdded'] })
const { result: { contexts: [{ context }] } } = await send('browsingContext.getTree', {})
await send('browsingContext.setViewport', { context, viewport: { width: 1440, height: 900 } })
const open = async (path) => { await send('browsingContext.navigate', { context, url: base + path, wait: 'complete' }) }
const run = async (body) => {
  const { result } = await send('script.evaluate', { expression: `(async () => { ${body} })()`, target: { context }, awaitPromise: true })
  return result.type === 'exception' ? `EXCEPTION ${result.exceptionDetails.text}` : result.result?.value
}
const until = async (body, ms = 20000) => { for (let t = 0; t < ms; t += 250) { if (await run(body)) return true; await sleep(250) } return false }

let failed = 0
const check = (name, ok, detail = '') => { if (!ok) failed++; console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${ok ? '' : `: ${detail}`}`) }

await open('/')
check('the landing page has one h1', (await run("return document.querySelectorAll('h1').length")) === 1)
const missing = (await fetch(base + '/no-such-page')).status
check('an unknown address is a 404', missing === 404, missing)

await open('/docs/model')
await sleep(1500)
const heavy = await run("return performance.getEntriesByType('resource').map((r) => r.name).filter((n) => /tapiti_bg\\.wasm/.test(n)).length")
check('docs load no wasm', heavy === 0, heavy)

// Accessibility, when axe-core is at hand: AXE_JS=/path/to/axe.min.js (CI installs it into a temp dir). No serious or critical violation.
if (process.env.AXE_JS) {
  const axe = readFileSync(process.env.AXE_JS, 'utf8')
  for (const path of ['/', '/docs/model', '/search?q=seed', '/lab']) {
    await open(path)
    await sleep(path === '/lab' ? 5000 : 1000)
    const found = await run(`${axe}\n; const r = await axe.run(document, { runOnly: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'] }); return JSON.stringify(r.violations.filter((v) => v.impact === 'serious' || v.impact === 'critical').map((v) => v.id + ' x' + v.nodes.length))`)
    const list = JSON.parse(found ?? '["no result"]')
    check(`axe: no serious or critical violation on ${path}`, Array.isArray(list) && list.length === 0, found)
  }
}

if (errors.length) { failed++; console.log('FAIL console errors:', errors.slice(0, 5)) }
ws.close()
firefox.kill()
process.exit(failed ? 1 : 0)
