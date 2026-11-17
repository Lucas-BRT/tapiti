// Smoke test of the lab in headless Firefox, over WebDriver BiDi (no driver to install; needs Node 22+ and `firefox`).
//   node scripts/lab-smoke.mjs [url]        default: http://127.0.0.1:3000/lab (start the site first: topcoat dev -p tapiti-site)
// Checks that the floor is drawn and the panel filled, that editing a parameter and a bad value behave, and that a benchmark runs.
import { spawn } from 'node:child_process'
import { freshProfile } from './firefox-profile.mjs'

const url = process.argv[2] ?? 'http://127.0.0.1:3000/lab'
const port = 9333
const profile = freshProfile('lab')
const firefox = spawn('firefox', ['--headless', '--no-remote', '--profile', profile, `--remote-debugging-port=${port}`], { stdio: 'ignore' })
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

let ws
for (let i = 0; i < 60 && !ws; i++) {
  try {
    ws = await new Promise((resolve, reject) => {
      const socket = new WebSocket(`ws://127.0.0.1:${port}/session`)
      socket.onopen = () => resolve(socket)
      socket.onerror = reject
    })
  } catch { await sleep(500) }
}
if (!ws) { firefox.kill(); console.error('could not reach Firefox'); process.exit(2) }

let id = 0
const waiting = new Map()
const logs = []
ws.onmessage = ({ data }) => {
  const message = JSON.parse(data)
  if (message.id && waiting.has(message.id)) waiting.get(message.id)(message)
  else if (message.method === 'log.entryAdded' && message.params.level === 'error') logs.push(message.params.text)
}
const send = (method, params = {}) => new Promise((resolve) => { const n = ++id; waiting.set(n, resolve); ws.send(JSON.stringify({ id: n, method, params })) })

await send('session.new', { capabilities: {} })
await send('session.subscribe', { events: ['log.entryAdded'] })
const { result: { contexts: [{ context }] } } = await send('browsingContext.getTree', {})
await send('browsingContext.setViewport', { context, viewport: { width: 1440, height: 900 } })
await send('browsingContext.navigate', { context, url, wait: 'complete' })
await sleep(2500)
const resize = (width, height) => send('browsingContext.setViewport', { context, viewport: { width, height } })
const run = async (body) => {
  const { result } = await send('script.evaluate', { expression: `(async () => { ${body} })()`, target: { context }, awaitPromise: true })
  return result.type === 'exception' ? `EXCEPTION ${result.exceptionDetails.text}` : result.result?.value
}

const pressOn = (target, init) => `${target}.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ${init} }))`
const wait = (ms) => `await new Promise(r => setTimeout(r, ${ms}))`
const checks = [
  ['panel is filled, with the rules and the hash', `return document.getElementById('results').innerText.includes('V1 connectivity') && /hash [0-9a-f]{8}/.test(document.getElementById('results').innerText)`, (v) => v === true],
  ['the measures use the shared words', `const t = document.getElementById('results').innerText; return ['single-door rooms', 'steps to exit', 'doors per room'].every((w) => t.includes(w)) && !/dead ends|exit hops|mean doors/.test(t)`, (v) => v === true],
  ['size button works', `document.querySelector('[data-size="3"]').click(); ${wait(900)}; return document.getElementById('results').innerText.includes('large size, seed')`, (v) => v === true],
  ['a parameter change is counted', `const el = document.querySelector('[name="cols"]'); el.value = '9'; el.dispatchEvent(new Event('change', { bubbles: true })); return document.getElementById('changed-status').textContent`, (v) => /1 parameter/.test(v)],
  ['a changed field is marked and can be put back', `const box = document.querySelector('[name="cols"]').closest('.field'); const reset = box.querySelector('.field-reset'); const marked = box.classList.contains('changed') && !reset.hidden; reset.click(); return marked + '|' + document.getElementById('changed-status').textContent`, (v) => v.startsWith('true|') && /Default/.test(v)],
  ['every group can be edited (zones, levels and environment too)', `const errors = []; window.addEventListener('error', (e) => errors.push(e.message)); let n = 0; for (const input of document.querySelectorAll('#parameter-groups [name]')) { if (input.dataset.kind === 'boolean') { input.checked = !input.checked } else if (input.dataset.kind === 'number') { input.value = input.min } else continue; input.dispatchEvent(new Event('change', { bubbles: true })); n++ } ${wait(600)}; return errors.length + '|' + n + '|' + document.querySelector('[name="environment.height.enabled"]').checked`, (v) => v.startsWith('0|') && Number(v.split('|')[1]) > 100],
  ['a bad value is refused, in words, and linked to its field', `document.querySelector('[data-size="1"]').click(); ${wait(300)}; const el = document.querySelector('[name="rows"]'); el.value = '999'; el.dispatchEvent(new Event('change', { bubbles: true })); const err = document.getElementById(el.getAttribute('aria-describedby')); return err.textContent + '|' + (document.getElementById('lab-messages').textContent.includes('Not applied'))`, (v) => /^Rows: use a whole number/.test(v) && v.endsWith('true')],
  ['an error message stays until it is closed', `${wait(7000)}; const m = document.querySelector('#lab-messages .lab-message.error'); const was = !!m; m?.querySelector('button').click(); return was + '|' + !!document.querySelector('#lab-messages .lab-message.error')`, (v) => v === 'true|false'],
  ['undo and redo are separate, with the keys', `document.querySelector('[data-size="1"]').click(); ${wait(400)}; const el = document.querySelector('[name="cols"]'); el.value = '7'; el.dispatchEvent(new Event('change', { bubbles: true })); const changed = document.getElementById('changed-status').textContent; document.body.focus(); ${pressOn('window', "key: 'z', ctrlKey: true")}; const undone = document.getElementById('changed-status').textContent; ${pressOn('window', "key: 'y', ctrlKey: true")}; const redone = document.getElementById('changed-status').textContent; return [changed, undone, redone].join('|')`, (v) => /^1 parameter changed\|Default configuration\|1 parameter changed$/.test(v)],
  ['Alt+2 picks the medium size; a bare R does nothing', `const seed = document.getElementById('seed-input').value; ${pressOn('document.querySelector(".size-button")', "key: 'r'")}; ${wait(200)}; const same = document.getElementById('seed-input').value === seed; ${pressOn('window', "key: '2', altKey: true")}; ${wait(300)}; return same + '|' + document.querySelector('[data-size="2"]').getAttribute('aria-pressed')`, (v) => v === 'true|true'],
  ['the plan can be used from the keyboard: pick, release', `const c = document.getElementById('floor-map'); c.focus(); ${pressOn('c', "key: ']'")}; const picked = document.getElementById('selected-room').innerText.length > 0; ${pressOn('c', "key: 'Escape'")}; return picked + '|' + (document.getElementById('selected-room').innerText.length === 0)`, (v) => v === 'true|true'],
  ['the exit is not amber, and the plan has a focus ring drawn inside it', `const s = getComputedStyle(document.getElementById('lab')); let ring = ''; for (const sheet of document.styleSheets) { try { for (const rule of sheet.cssRules) { if (rule.selectorText === '#floor-map:focus-visible') ring = rule.style.outlineOffset } } catch {} } return (s.getPropertyValue('--role-exit') !== s.getPropertyValue('--amber')) + '|' + ring`, (v) => /^true\|-\d/.test(v)],
  ['environment layers can be drawn: a tint to choose from', `document.querySelector('[name="environment.height.enabled"]').checked = true; document.querySelector('[name="environment.height.enabled"]').dispatchEvent(new Event('change', { bubbles: true })); ${wait(900)}; const r = document.querySelector('input[name=tint][value=height]'); return !r.disabled`, (v) => v === true],
  ['the settings dialog opens, scales the text, and stops the motion', `document.querySelector('.settings-button').click(); const open = document.getElementById('settings-dialog').open; const set = (name, value) => { const i = document.querySelector('input[name="' + name + '"][value="' + value + '"]'); i.checked = true; i.dispatchEvent(new Event('change', { bubbles: true })) }; set('text', 'large'); set('motion', 'off'); const zoom = document.documentElement.style.zoom; const motion = document.documentElement.dataset.motion; set('text', 'normal'); document.getElementById('settings-reset').click(); document.getElementById('settings-dialog').close(); return open + '|' + zoom + '|' + motion`, (v) => v === 'true|1.15|off'],
  ['benchmark runs', `document.querySelector('details.tool-group:last-of-type summary')?.click(); document.getElementById('bench-40').click(); return document.getElementById('bench-status').textContent`, (v) => /invalid of 40/.test(v)],
]
let failed = 0
const report = (name, pass, value) => {
  if (!pass) failed++
  console.log(`${pass ? 'ok  ' : 'FAIL'} ${name}${pass ? '' : `: ${JSON.stringify(value)}`}`)
}
for (const [name, body, ok] of checks) {
  const value = await run(body)
  report(name, ok(value), value)
}

// The reading drawer (below 1280px): opens, takes the focus, closes on Escape and gives the focus back.
await resize(1100, 800)
await sleep(300)
report('the reading drawer opens', (await run("document.getElementById('reading-toggle').click(); await new Promise(r => setTimeout(r, 300)); return document.getElementById('reading-toggle').getAttribute('aria-expanded') + '|' + (document.activeElement?.id)")) === 'true|results', 'drawer')
report('Escape closes the drawer and returns the focus', (await run("document.getElementById('lab').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); await new Promise(r => setTimeout(r, 300)); return document.getElementById('reading-toggle').getAttribute('aria-expanded') + '|' + (document.activeElement?.id)")) === 'false|reading-toggle', 'escape')

report('the shape legend repeats the plan hatches', (await run("return document.querySelectorAll('.legend-list [data-shape]').length")) === 5, 'legend')

await resize(1440, 900)
await sleep(300)

// Pocket: a short screen (a phone held sideways, a page at 200% zoom) still reaches the form, and a phone held upright leaves room for it.
for (const [w, h, name] of [[640, 400, 'at 200% zoom'], [812, 375, 'on a phone held sideways'], [375, 667, 'on a phone held upright']]) {
  await resize(w, h)
  await sleep(500)
  const got = await run("const v = document.getElementById('view').getBoundingClientRect(); const t = document.querySelector('.lab-tabs').getBoundingClientRect(); return JSON.stringify({ view: Math.round(v.height), tabs: Math.round(t.top), h: innerHeight })")
  const { view, tabs, h: inner } = JSON.parse(got)
  report(`the tabs and the form are reachable ${name}`, view <= inner * 0.62 && tabs < inner, got)
}

// No sideways scroll on a phone.
await resize(390, 844)
await sleep(600)
report('no horizontal scroll at 390px', (await run('return document.documentElement.scrollWidth <= document.documentElement.clientWidth')) === true, 'overflow')

// The old address still leads here.
const moved = await fetch(new URL('/en/lab', url), { redirect: 'manual' })
report('/en/lab redirects to /lab', moved.status === 308 && new URL(moved.headers.get('location'), url).pathname === '/lab', moved.status)
if (logs.length) { failed++; console.log('FAIL console errors:', logs) }
ws.close()
firefox.kill()
process.exit(failed ? 1 : 0)
