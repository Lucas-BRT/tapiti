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

// The docs: each page says its own description and has its own title; narrow screens get the page's outline; the parameter reference
// has a row to link to for each parameter and its defaults; the search page is not indexed; the 404 offers the lab and the search.
const meta = async (path) => (await run(`const r = await fetch('${path}'); const t = await r.text(); const d = new DOMParser().parseFromString(t, 'text/html'); return [d.title, d.querySelector('meta[name=description]')?.content ?? '', d.querySelector('meta[name=robots]')?.content ?? ''].join('|')`))
const [m1, m2] = [await meta('/docs/getting-started'), await meta('/docs/model')]
check('each docs page has its own title and description', m1 !== m2 && m1.split('|')[0].endsWith('· Tapiti') && m1.split('|')[1] !== m2.split('|')[1], [m1, m2])
check('the search page is not indexed', /noindex/.test(await meta('/search?q=seed')))
check('the lab has its full title', (await meta('/lab')).startsWith('Lab · Tapiti'))
await open('/docs/parameters')
check('the parameter reference has a row and a default for each parameter', (await run("return document.querySelectorAll('tr[id^=param-]').length > 100 && document.querySelector('th:nth-child(4)').textContent === 'Default'")) === true)
await open('/docs/model')
await run("return true")
const notFound = await (await fetch(base + '/no-such-page')).text()
check('the 404 offers the lab and the search', notFound.includes('href="/lab"') && notFound.includes('/search'))
await open('/')
check('the hero has IN and OUT chips and no V1 to V5 strip', (await run("return !!document.querySelector('.tapiti-floor .badges') && !document.querySelector('.bench-bar').textContent.includes('V1 V2')")) === true)
check('the parameter tree links to its groups', (await run("return new Set([...document.querySelectorAll('.tree-list a')].map((a) => a.hash)).size > 5")) === true)

// Settings: the button opens a dialog; the theme, the text size and the motion are choices of presets (and a value of one's own),
// kept in the browser, applied at once and on the page before it is painted. They start from the system's.
await open('/')
await run("localStorage.clear(); document.documentElement.removeAttribute('data-theme'); document.documentElement.style.zoom = ''")
await open('/')
check('the settings button is shown', (await run("return !document.querySelector('.settings-button').hidden")) === true)
const choose = (name, value) => run(`const i = document.querySelector('input[name="${name}"][value="${value}"]'); i.checked = true; i.dispatchEvent(new Event('change', { bubbles: true })); return true`)
await run("document.querySelector('.settings-button').click()")
check('the dialog opens', (await run("return document.getElementById('settings-dialog').open")) === true)
check('the theme starts from the system', (await run("return document.querySelector('input[name=theme][value=system]').checked")) === true)
await choose('theme', 'light')
check('a theme choice is applied at once', (await run("return document.documentElement.dataset.theme")) === 'light')
await choose('theme', 'dark')
check('and kept', (await run("return JSON.parse(localStorage.getItem('tapiti.settings')).theme + '|' + localStorage.getItem('tapiti-theme')")) === 'dark|dark')
await choose('text', 'larger')
check('the text size scales the page', (await run("return document.documentElement.style.zoom")) === '1.3')
await choose('motion', 'custom')
await run("const n = document.querySelector('input[data-custom=motion]'); n.value = '300'; n.dispatchEvent(new Event('input', { bubbles: true })); return true")
check('a custom motion time is a value of its own', (await run("return getComputedStyle(document.documentElement).getPropertyValue('--t').trim()")) === '300ms')
await choose('motion', 'off')
check('motion off', (await run("return document.documentElement.dataset.motion")) === 'off')
await run("document.getElementById('settings-dialog').close()")
await open('/')
check('a chosen theme and text size are on the page before it is painted', (await run("return document.documentElement.dataset.theme + '|' + document.documentElement.style.zoom + '|' + document.documentElement.dataset.motion")) === 'dark|1.3|off')
await run("document.querySelector('.settings-button').click(); document.getElementById('settings-reset').click(); document.getElementById('settings-dialog').close(); return true")
check('back to the system choices', (await run("return (document.documentElement.dataset.theme ?? 'none') + '|' + (localStorage.getItem('tapiti.settings') ?? 'none')")) === 'none|none')
await run("localStorage.clear()")

// Accessibility, when axe-core is at hand: AXE_JS=/path/to/axe.min.js (CI installs it into a temp dir). No serious or critical violation,
// in the light theme and in the dark one.
if (process.env.AXE_JS) {
  const axe = readFileSync(process.env.AXE_JS, 'utf8')
  for (const path of ['/', '/docs/model', '/search?q=seed', '/lab']) {
    for (const theme of ['light', 'dark']) {
      await open(path)
      await run(`document.documentElement.dataset.theme = '${theme}'`)
      await sleep(path === '/lab' ? 5000 : 1000)
      const found = await run(`${axe}\n; const r = await axe.run(document, { runOnly: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'] }); return JSON.stringify(r.violations.filter((v) => v.impact === 'serious' || v.impact === 'critical').map((v) => v.id + ' x' + v.nodes.length))`)
      const list = JSON.parse(found ?? '["no result"]')
      check(`axe: no serious or critical violation on ${path} (${theme})`, Array.isArray(list) && list.length === 0, found)
    }
  }
}

if (errors.length) { failed++; console.log('FAIL console errors:', errors.slice(0, 5)) }
ws.close()
firefox.kill()
process.exit(failed ? 1 : 0)
