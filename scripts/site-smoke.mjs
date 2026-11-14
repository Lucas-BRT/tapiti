// Smoke test of the landing page's walk in headless Firefox, over WebDriver BiDi (Node 22+ and `firefox`; start the site first).
//   node scripts/site-smoke.mjs [base-url]        default: http://127.0.0.1:3000
// Checks the scene starts and stays inside its budgets, follows the seven rooms, keeps paper signs readable,
// can be switched off, steps under reduced motion, and that the docs load neither three.js nor wasm. With AXE_JS set, also runs axe-core.
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

// The native reading view stays available without WebGL.
await open('/?scene=off')
await sleep(800)
check('?scene=off keeps the map', (await run("return !document.documentElement.classList.contains('walk-on') && !window.__walk")) === true)

const webgl = await run("const c = document.createElement('canvas'); let why = ''; c.addEventListener('webglcontextcreationerror', (e) => { why = e.statusMessage }); const gl = c.getContext('webgl2'); return gl ? 'yes' : 'no: ' + (why || 'no reason given')")
check('WebGL2 is available in this Firefox', webgl === 'yes', `${webgl}; the scene and the lab 3D view need it`)
await open('/')
const started = await until("return document.documentElement.classList.contains('walk-on')")
check('the scene starts', started)
const w = JSON.parse((await run('return JSON.stringify(window.__walk && { ...window.__walk, stations: window.__walk.stations.map((s) => [s.x, s.y, s.u]) })')) ?? 'null') ?? {}
check('draw calls <= 40', w.calls <= 40, w.calls)
check('triangles <= 60000', w.triangles <= 60000, w.triangles)
check('the paper walk needs no lights', w.lights === 0, w.lights)
const rooms = JSON.parse(await run("return JSON.stringify([...document.querySelectorAll('.rooms [data-station]')].map((s) => Number(s.dataset.station)))"))
check('the seven HTML rooms match the scene stations', w.stations?.length === 7 && rooms.length === 7 && rooms.every((n, i) => n === i))
check('world signs retain the original content', (await run("return document.querySelectorAll('.world-rig .room-body').length")) === 7)
check('signs have opaque paper', (await run("return getComputedStyle(document.querySelector('.world-rig .room-body')).backgroundColor")) === 'rgb(255, 252, 243)')
check('Caveat is bundled and loaded', (await run("await document.fonts.ready; return document.fonts.check('600 24px Caveat')")) === true)

for (const [i, section] of ['entrance', 'shapes', 'gate', 'dig', 'numbers', 'run', 'stairs'].entries()) {
  await run(`const s = document.getElementById('${section}'); window.scrollTo({ top: s.offsetTop, behavior: 'instant' })`)
  const reached = await until(`return Math.abs(window.__walk.camera.u - ${w.stations?.[i]?.[2] ?? 0}) < 0.001`, 8000)
  check(`scrolling to "${section}" brings the camera there`, reached, await run('return JSON.stringify(window.__walk.camera)'))
}
await open('/?motion=reduce#gate')
await until("return document.documentElement.classList.contains('walk-on')")
check('reduced motion steps the camera', (await run('return window.__walk && window.__walk.stepped')) === true)
check('the gate deep link survives the scene layout', await until(`return Math.abs(window.__walk.camera.u - ${w.stations?.[2]?.[2] ?? 0}) < 0.001`))
check('the seed sign can receive keyboard focus', (await run("const input = document.querySelector('.world-rig input[name=seed]'); input?.focus(); return !!input && document.activeElement === input")) === true)

// A dead end, and every address that leads nowhere, in the same room (with the right status).
await open('/leaf')
const robots = await run("return document.querySelector('meta[name=robots]')?.content")
check('/leaf is noindex', /noindex/.test(robots ?? ''), robots)
const leafOn = await until("return document.documentElement.classList.contains('walk-on')")
check('the leaf chamber draws its room', leafOn && (await run('return window.__walk && window.__walk.torches')) === 1)
for (const path of ['/nope', '/a/b/c', '/docs/nope']) {
  const status = await run(`return (await fetch('${path}')).status`)
  const title = await run(`return (await (await fetch('${path}')).text()).includes('Nothing here')`)
  check(`${path} is a 404 with the leaf chamber`, status === 404 && title === true, `${status}`)
}
await open('/leaf?motion=reduce')
await until("return document.documentElement.classList.contains('walk-on')")
check('reduced motion keeps the leaf chamber still', (await run('return window.__walk && window.__walk.stepped')) === true)

await open('/docs/model')
await sleep(1500)
const heavy = await run("return performance.getEntriesByType('resource').map((r) => r.name).filter((n) => /three-subset|tapiti_bg\\.wasm|scene\\.js/.test(n)).length")
check('docs load no three.js and no wasm', heavy === 0, heavy)

// Accessibility, when axe-core is at hand: AXE_JS=/path/to/axe.min.js (CI installs it into a temp dir). No serious or critical violation.
if (process.env.AXE_JS) {
  const axe = readFileSync(process.env.AXE_JS, 'utf8')
  for (const path of ['/', '/docs/model', '/search?q=seed', '/leaf', '/lab']) {
    await open(path)
    await sleep(path === '/lab' || path === '/' || path === '/leaf' ? 5000 : 1000)
    const found = await run(`${axe}\n; const r = await axe.run(document, { runOnly: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'] }); return JSON.stringify(r.violations.filter((v) => v.impact === 'serious' || v.impact === 'critical').map((v) => v.id + ' x' + v.nodes.length))`)
    const list = JSON.parse(found ?? '["no result"]')
    check(`axe: no serious or critical violation on ${path}`, Array.isArray(list) && list.length === 0, found)
  }
}

if (errors.length) { failed++; console.log('FAIL console errors:', errors.slice(0, 5)) }
ws.close()
firefox.kill()
process.exit(failed ? 1 : 0)
