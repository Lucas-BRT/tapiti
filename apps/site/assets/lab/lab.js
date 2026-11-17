// The browser lab. The server (Topcoat) renders every control, label and help text; this module binds to that markup and drives the
// WebAssembly generator (in a worker, so typing never waits for a floor), a pool of workers for batches, and the canvas. Text it builds
// while running comes from `data-i18n`. The desktop app and this page read a floor with the same words (`tapiti-render` writes them).

const root = document.getElementById('lab')
const $ = (id) => document.getElementById(id)
const dict = JSON.parse(root.dataset.i18n)
const ISOLATES = /[⁦-⁩]/g // Fluent wraps values in invisible direction marks; a template needs none
const tr = (id, args = {}) => (dict[id] ?? id).replace(ISOLATES, '').replace(/\{(\w+)\}/g, (_, key) => args[key] ?? `{${key}}`)
const formatNumber = (n, digits = 1) => Number(n).toLocaleString('en-US', { maximumFractionDigits: digits })
const clone = (value) => JSON.parse(JSON.stringify(value))
const getPath = (object, path) => path.split('.').reduce((value, key) => value?.[key], object)
// A path can run through a group the floor's Λ leaves out while it is off (zones, levels, environment): the objects are made on the way.
const setPath = (object, path, value) => {
  const keys = path.split('.')
  const last = keys.pop()
  keys.reduce((target, key) => (target[key] ??= {}), object)[last] = value
}
const remember = (key, value) => {
  try {
    if (value === undefined) return localStorage.getItem(key)
    if (value === null) return localStorage.removeItem(key)
    localStorage.setItem(key, value)
  } catch {
    return null
  }
}
const el = (tag, attrs = {}, ...kids) => {
  const node = document.createElement(tag)
  for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value)
  node.append(...kids)
  return node
}

/* ---------------- Loading the module ---------------- */

// The module is loaded here, not imported at the top, so that a failed download is told to the person (with a way to try again)
// instead of leaving a page that says "Loading" for ever.
let wasm
let site
const showLoadFailure = (error) => {
  root.setAttribute('aria-busy', 'false')
  $('results-body').replaceChildren(
    el('div', { class: 'validity-card invalid', role: 'alert' },
      el('div', {},
        el('strong', {}, tr('lab_load_failed_title')),
        el('p', {}, tr('lab_load_failed_detail', { error: String(error?.message ?? error) })),
        el('button', { type: 'button', class: 'secondary-button', id: 'lab-reload' }, tr('lab_reload')))))
  $('lab-reload').addEventListener('click', () => location.reload())
  $('controls').inert = true
  root.dataset.tab = 'reading'
}
try {
  wasm = await import('/lab/pkg/tapiti.js')
  await wasm.default()
} catch (error) {
  showLoadFailure(error)
  throw error
}
site = await import(root.dataset.settings).catch(() => null)
const { full_params, stage_json, bench, inapplicable } = wasm
const { hatchPatterns } = await import(root.dataset.hatch)

const setting = () => site?.resolve?.() ?? { shortcuts: true, memoryRooms: Infinity, reading: 'automatic', readingWidth: 328 }

/* ---------------- Workers ---------------- */

// Each worker has its own single-threaded instance of the module. A stage is cheap (a few ms to some tens), so the parallelism that
// pays in the browser is across stages: a batch is cut into slices, one per worker, with no shared memory and no special headers.
const createPool = (size = Math.max(1, Math.min(navigator.hardwareConcurrency || 4, 16))) => {
  const workers = Array.from({ length: size }, () => new Worker(root.dataset.worker, { type: 'module' }))
  let next = 0
  const pending = new Map()
  for (const w of workers) {
    w.onmessage = ({ data }) => {
      pending.get(data.id)?.(data)
      pending.delete(data.id)
    }
  }
  const batch = async (preset, from, count, use = size) => {
    const t0 = performance.now()
    const n = Math.max(1, Math.min(use, size, count))
    const slices = Array.from({ length: n }, (_, i) => [from + Math.floor((count * i) / n), Math.floor((count * (i + 1)) / n) - Math.floor((count * i) / n)])
    const answers = await Promise.all(
      slices.map(([f, c], i) => new Promise((resolve) => {
        const id = next++
        pending.set(id, resolve)
        workers[i].postMessage({ id, size: preset, from: f, count: c })
      })),
    )
    return { invalid: answers.reduce((a, r) => a + r.invalid, 0), ms: performance.now() - t0, workers: n }
  }
  return { batch }
}
let pool = null

// One more worker draws the floors, so a huge floor never freezes the page. If it cannot start, the page's own copy of the module does it.
let stageWorker = null
let stageId = 0
const stagePending = new Map()
const startStageWorker = () => {
  try {
    const w = new Worker(root.dataset.worker, { type: 'module' })
    w.onmessage = ({ data }) => {
      const done = stagePending.get(data.id)
      stagePending.delete(data.id)
      done?.(data)
    }
    w.onerror = () => {
      stageWorker = false
      for (const [id, done] of stagePending) { stagePending.delete(id); done({ id, fallback: true }) }
    }
    return w
  } catch {
    return false
  }
}
const stageAsync = (seed, params) => {
  if (stageWorker === null) stageWorker = startStageWorker()
  if (!stageWorker) return Promise.resolve().then(() => stage_json(seed, params))
  const id = stageId++
  return new Promise((resolve, reject) => {
    stagePending.set(id, (data) => {
      if (data.fallback) return resolve(stage_json(seed, params))
      if (data.error) reject(new Error(data.error))
      else resolve(data.json)
    })
    stageWorker.postMessage({ type: 'stage', id, seed, params })
  })
}

/* ---------------- Constants and state ---------------- */

const CLASSES = ['rect', 'disc', 'oct', 'L', 'star']
const SIZE_NAMES = ['small', 'medium', 'large', 'huge']
const className = (name) => dict[`lab_class_${name}`] ?? name
const roleName = (name) => dict[`lab_role_${name}`] ?? name
const DIRS = [[1, 0], [Math.SQRT1_2, Math.SQRT1_2], [0, 1], [-Math.SQRT1_2, Math.SQRT1_2], [-1, 0], [-Math.SQRT1_2, -Math.SQRT1_2], [0, -1], [Math.SQRT1_2, -Math.SQRT1_2]]

// The Λ with every group present, so every field of the form has a value to show and a place to write to.
const fullParams = (size) => {
  const P = JSON.parse(full_params(size))
  if (getPath(P, 'topology.minDetours') === undefined) setPath(P, 'topology.minDetours', 0)
  return P
}
const state = { size: 1, seed: 1, P: fullParams(1), defaults: fullParams(1), tint: 'shape', level: 0, selected: null }
const overlays = { cells: true, labels: false, ports: false, pairs: false, candidates: false }
const view = { zoom: 1, x: 0, y: 0 }
let current = null
let regenTimer = 0
let generation = 0
let pending = false
let stale = false

/* ---------------- Colours of the page, read once per theme ---------------- */

const rootStyle = () => getComputedStyle(root)
let cssCache = new Map()
const css = (name) => {
  if (!cssCache.has(name)) cssCache.set(name, rootStyle().getPropertyValue(name).trim())
  return cssCache.get(name)
}
const dropColours = () => { cssCache = new Map(); draw() }

/* ---------------- Messages ---------------- */

// A message is not a progress line. An error stays until it is closed; an answer fades. Both are read out; an error interrupts.
const messages = $('lab-messages')
const live = $('lab-live')
const announce = (text) => { live.textContent = ''; setTimeout(() => { live.textContent = text }, 30) }
const closeMessage = (node) => node.remove()
const notify = (text, { error = false, key = '' } = {}) => {
  if (key) for (const old of messages.querySelectorAll(`[data-key="${key}"]`)) old.remove()
  const node = el('div', { class: `lab-message ${error ? 'error' : 'info'}`, role: error ? 'alert' : 'status' },
    el('span', {}, text),
    el('button', { type: 'button', class: 'message-close', 'aria-label': tr('lab_msg_close') }, '×'))
  if (key) node.dataset.key = key
  node.querySelector('button').addEventListener('click', () => closeMessage(node))
  messages.append(node)
  if (!error) setTimeout(() => node.remove(), 6000)
  return node
}
const clearMessage = (key) => { for (const old of messages.querySelectorAll(`[data-key="${key}"]`)) old.remove() }

/* ---------------- Help tooltips ---------------- */

const helpTooltip = $('help-tooltip')
let activeHelp = null

const hideHelp = () => {
  activeHelp?.setAttribute('aria-expanded', 'false')
  activeHelp = null
  helpTooltip.hidden = true
}
const showHelp = (button, pinned = false) => {
  if (activeHelp && activeHelp !== button) activeHelp.setAttribute('aria-expanded', 'false')
  activeHelp = button
  helpTooltip.textContent = button.dataset.help
  helpTooltip.hidden = false
  button.setAttribute('aria-expanded', pinned ? 'true' : 'false')
  const rect = button.getBoundingClientRect()
  const width = Math.min(320, window.innerWidth - 24)
  helpTooltip.style.width = `${width}px`
  helpTooltip.style.left = `${Math.max(12, Math.min(rect.left, window.innerWidth - width - 12))}px`
  const below = rect.bottom + 8 + helpTooltip.offsetHeight < window.innerHeight
  helpTooltip.style.top = `${below ? rect.bottom + 8 : Math.max(12, rect.top - helpTooltip.offsetHeight - 8)}px`
}
const helpOf = (event) => event.target.closest?.('.help-button')
root.addEventListener('mouseover', (event) => { const b = helpOf(event); if (b) showHelp(b) })
root.addEventListener('focusin', (event) => { const b = helpOf(event); if (b) showHelp(b) })
root.addEventListener('mouseout', (event) => { const b = helpOf(event); if (b && activeHelp === b && document.activeElement !== b) hideHelp() })
root.addEventListener('focusout', (event) => { const b = helpOf(event); if (b && activeHelp === b && b.getAttribute('aria-expanded') !== 'true') hideHelp() })
root.addEventListener('click', (event) => {
  const b = helpOf(event)
  if (!b) return hideHelp()
  event.preventDefault()
  if (activeHelp === b && b.getAttribute('aria-expanded') === 'true') hideHelp()
  else showHelp(b, true)
})
// A help stays while its button has the focus (scrolling the panel to the button must not hide it); Esc closes it.
window.addEventListener('scroll', () => { if (activeHelp && document.activeElement !== activeHelp) hideHelp() }, true)
root.addEventListener('keydown', (event) => { if (event.key === 'Escape' && activeHelp) { const b = activeHelp; hideHelp(); b.focus(); event.stopPropagation() } }, true)

/* ---------------- Parameter form ---------------- */

const form = $('parameter-groups')
const fields = [...form.querySelectorAll('[name]')]
const fieldBox = (input) => input.closest('.field')
const fieldLabel = (input) => fieldBox(input).querySelector('label').textContent.trim()
const sizeDescription = $('size-description')
const changedStatus = $('changed-status')
const seedInput = $('seed-input')
const sizeButtons = [...root.querySelectorAll('.size-button')]

const valueText = (value) => (typeof value === 'boolean' ? (value ? 'on' : 'off') : String(value))
const isChanged = (el) => JSON.stringify(getPath(state.P, el.name)) !== JSON.stringify(getPath(state.defaults, el.name))

const syncForm = () => {
  for (const input of fields) {
    const value = getPath(state.P, input.name)
    if (input.dataset.kind === 'boolean') input.checked = !!value
    else input.value = value === undefined ? '' : String(value)
    input.removeAttribute('aria-invalid')
    delete fieldBox(input).dataset.unapplied
    fieldBox(input).querySelector('.field-error').textContent = ''
  }
  const junction = state.P.junction
  $('junction-value').textContent = junction == null ? tr('lab_junction_inf') : formatNumber(junction, 2)
  updateChanged()
}

const updateChanged = () => {
  let n = 0
  for (const input of fields) {
    const changed = isChanged(input)
    const box = fieldBox(input)
    box.classList.toggle('changed', changed)
    const reset = box.querySelector('.field-reset')
    reset.hidden = !changed
    if (changed) {
      n++
      const was = valueText(getPath(state.defaults, input.name))
      reset.setAttribute('aria-label', tr('lab_reset_field', { label: fieldLabel(input), value: was }))
      reset.title = tr('lab_changed_field', { value: was })
    }
  }
  changedStatus.textContent = n === 0 ? tr('lab_changed_none') : n === 1 ? tr('lab_changed_one') : tr('lab_changed_some', { n })
  changedStatus.classList.toggle('changed', n > 0)
  for (const group of form.querySelectorAll('details.parameter-group[data-group]')) {
    const count = group.querySelector('.group-count')
    const mine = [...group.querySelectorAll('[name]')].filter(isChanged).length
    count.textContent = mine ? tr('lab_changed_group', { n: mine, total: count.dataset.total }) : count.dataset.total
    count.classList.toggle('changed', mine > 0)
  }
}

// Fields that can change nothing under the present Λ are marked, as the desktop app does not list them: the rules are the same ones.
const markUnused = () => {
  let unused = []
  try { unused = JSON.parse(inapplicable(JSON.stringify(state.P), JSON.stringify(fields.map((f) => f.name)))) } catch { /* the page keeps working */ }
  const set = new Set(unused)
  for (const input of fields) {
    const box = fieldBox(input)
    const off = set.has(input.name)
    box.classList.toggle('unused', off)
    box.querySelector('.field-note').textContent = off ? tr('lab_not_used') : ''
  }
}

const setFieldValue = (input, value) => setPath(state.P, input.name, value)

form.addEventListener('change', (event) => {
  const input = event.target
  if (!input.name) return
  const label = fieldLabel(input)
  rememberForEdit()
  if (input.dataset.kind === 'boolean') setFieldValue(input, input.checked)
  else if (input.dataset.kind === 'choice') setFieldValue(input, typeof getPath(state.P, input.name) === 'number' ? Number(input.value) : input.value)
  else {
    const value = Number(input.value)
    const min = Number(input.min)
    const max = Number(input.max)
    const integer = input.dataset.integer !== undefined
    const error = fieldBox(input).querySelector('.field-error')
    if (!input.value || !Number.isFinite(value) || value < min || value > max || (integer && !Number.isInteger(value))) {
      const text = tr(integer ? 'lab_err_field_int' : 'lab_err_field_range', { label, min: formatNumber(min, 2), max: formatNumber(max, 2) })
      error.textContent = text
      input.setAttribute('aria-invalid', 'true')
      fieldBox(input).dataset.unapplied = 'true'
      notify(text, { error: true, key: `field-${input.name}` })
      return
    }
    error.textContent = ''
    input.removeAttribute('aria-invalid')
    delete fieldBox(input).dataset.unapplied
    clearMessage(`field-${input.name}`)
    setFieldValue(input, value)
  }
  changed()
})

form.addEventListener('click', (event) => {
  const groupReset = event.target.closest('.group-reset')
  const fieldReset = event.target.closest('.field-reset')
  if (!groupReset && !fieldReset) return
  remember_()
  const inputs = groupReset ? [...groupReset.closest('details').querySelectorAll('[name]')] : [fieldBox(fieldReset).querySelector('[name]')]
  for (const input of inputs) setPath(state.P, input.name, clone(getPath(state.defaults, input.name)))
  syncForm()
  changed()
  if (fieldReset) fieldBox(fieldReset).querySelector('[name]').focus()
})

function setSize(size, { remember: keep = true } = {}) {
  if (keep) remember_()
  state.size = size
  state.P = fullParams(size)
  state.defaults = clone(state.P)
  syncForm()
  updateSizeButtons()
  changed()
}

const updateSizeButtons = () => {
  sizeButtons.forEach((button, index) => {
    const selected = index + 1 === state.size
    button.classList.toggle('selected', selected)
    button.setAttribute('aria-pressed', String(selected))
  })
  sizeDescription.textContent = tr(`lab_size_${state.size}`)
}

const changed = () => {
  updateChanged()
  scheduleRegenerate()
}

for (const button of sizeButtons) button.addEventListener('click', () => setSize(Number(button.dataset.size)))
$('reset-size').addEventListener('click', () => setSize(state.size))

// The filter: only the parameters that match stay in the form.
const filterInput = $('param-filter')
const filterStatus = $('filter-status')
const openBefore = new Map()
filterInput.addEventListener('input', () => {
  const needle = filterInput.value.trim().toLowerCase()
  let shown = 0
  for (const group of form.querySelectorAll('details.parameter-group[data-group]')) {
    if (needle && !openBefore.has(group)) openBefore.set(group, group.open)
    let any = 0
    for (const box of group.querySelectorAll('.field')) {
      const hit = !needle || box.textContent.toLowerCase().includes(needle) || (box.dataset.path ?? '').toLowerCase().includes(needle)
      box.hidden = !hit
      if (hit) any++
    }
    group.hidden = needle !== '' && any === 0
    if (needle) group.open = any > 0
    else if (openBefore.has(group)) group.open = openBefore.get(group)
    shown += needle ? any : 0
  }
  if (!needle) openBefore.clear()
  filterStatus.textContent = needle && shown === 0 ? tr('lab_filter_none') : ''
})

/* ---------------- Undo and redo ---------------- */

// One step each way, like the desktop app. The floor of the step is kept in memory (as Settings allow) so Undo does not generate it again.
const history = { previous: null, redo: null }
let lastRemembered = 0
const roomCount = (floor) => floor?.floor.rooms.filter((r) => r.alive).length ?? 0
const snapshot = () => ({
  size: state.size,
  seed: state.seed,
  P: clone(state.P),
  floor: !pending && !stale && current && roomCount(current) <= setting().memoryRooms ? current : null,
})
const updateHistoryButtons = () => {
  $('undo').disabled = !history.previous
  $('redo').disabled = !history.redo
}
function remember_() {
  history.previous = snapshot()
  history.redo = null
  lastRemembered = Date.now()
  updateHistoryButtons()
}
function rememberForEdit() {
  if (Date.now() - lastRemembered > 1500) remember_()
  lastRemembered = Date.now()
}
const applySnapshot = (snap) => {
  state.size = snap.size
  state.seed = snap.seed
  state.P = clone(snap.P)
  state.defaults = fullParams(snap.size)
  seedInput.value = String(snap.seed)
  syncForm()
  updateSizeButtons()
  if (snap.floor) {
    clearTimeout(regenTimer)
    generation++
    pending = false
    stale = false
    setCurrent(snap.floor)
  } else scheduleRegenerate()
  saveState()
}
const undo = () => {
  if (!history.previous) return notify(tr('lab_nothing_undo'))
  history.redo = snapshot()
  const snap = history.previous
  history.previous = null
  applySnapshot(snap)
  updateHistoryButtons()
  notify(tr('lab_undo_done'))
}
const redo = () => {
  if (!history.redo) return notify(tr('lab_nothing_redo'))
  history.previous = snapshot()
  const snap = history.redo
  history.redo = null
  applySnapshot(snap)
  updateHistoryButtons()
  notify(tr('lab_redo_done'))
}
$('undo').addEventListener('click', undo)
$('redo').addEventListener('click', redo)

/* ---------------- Seed, copy, paste, overlays, examples ---------------- */

const seedStep = (amount) => {
  state.seed = Math.max(0, Math.min(0xffffffff, state.seed + amount))
  seedInput.value = String(state.seed)
  scheduleRegenerate()
}
const newSeed = () => {
  state.seed = Math.floor(Math.random() * 1e6)
  seedInput.value = String(state.seed)
  scheduleRegenerate()
}
for (const button of root.querySelectorAll('[data-seed-step]')) button.addEventListener('click', () => seedStep(Number(button.dataset.seedStep)))
$('seed-random').addEventListener('click', newSeed)
seedInput.addEventListener('change', () => {
  const seed = Number(seedInput.value)
  if (!Number.isInteger(seed) || seed < 0 || seed > 0xffffffff) {
    seedInput.setAttribute('aria-invalid', 'true')
    notify(tr('lab_seed_error'), { error: true, key: 'seed' })
    return
  }
  seedInput.removeAttribute('aria-invalid')
  clearMessage('seed')
  state.seed = seed
  scheduleRegenerate()
})

const exportText = () => JSON.stringify({ size: SIZE_NAMES[state.size - 1], seed: state.seed, params: state.P }, null, 2)
$('copy-params').addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(exportText())
    notify(tr('lab_copied'))
  } catch {
    notify(tr('lab_no_clipboard'), { error: true })
  }
})

// The link holds the size, the seed and only what differs from the size's defaults.
const differences = () => {
  const diff = {}
  for (const input of fields) if (isChanged(input)) diff[input.name] = getPath(state.P, input.name)
  return diff
}
const toBase64Url = (text) => btoa(String.fromCharCode(...new TextEncoder().encode(text))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
const fromBase64Url = (text) => new TextDecoder().decode(Uint8Array.from(atob(text.replace(/-/g, '+').replace(/_/g, '/')), (c) => c.charCodeAt(0)))
const shareUrl = () => {
  const url = new URL(location.href)
  url.search = ''
  url.searchParams.set('size', SIZE_NAMES[state.size - 1])
  url.searchParams.set('seed', String(state.seed))
  const diff = differences()
  if (Object.keys(diff).length) url.searchParams.set('p', toBase64Url(JSON.stringify(diff)))
  return url.toString()
}
$('copy-link').addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(shareUrl())
    notify(tr('lab_link_copied'))
  } catch {
    notify(tr('lab_no_clipboard'), { error: true })
  }
})

const pasteDialog = $('paste-dialog')
const merge = (target, source) => {
  for (const [key, value] of Object.entries(source)) {
    if (value && typeof value === 'object' && !Array.isArray(value) && target[key] && typeof target[key] === 'object') merge(target[key], value)
    else if (key in target || target[key] === undefined) target[key] = value
  }
}
const applyPasted = (text) => {
  const data = JSON.parse(text)
  const params = data.params ?? data
  const size = SIZE_NAMES.indexOf(String(data.size ?? '').toLowerCase()) + 1 || state.size
  if (typeof params !== 'object' || params === null) throw new Error('not a set of parameters')
  remember_()
  state.size = size
  state.defaults = fullParams(size)
  state.P = fullParams(size)
  merge(state.P, params)
  if (Number.isInteger(data.seed) && data.seed >= 0 && data.seed <= 0xffffffff) { state.seed = data.seed; seedInput.value = String(data.seed) }
  syncForm()
  updateSizeButtons()
  changed()
}
$('paste-params').addEventListener('click', () => { $('paste-text').value = ''; pasteDialog.showModal(); $('paste-text').focus() })
$('paste-apply').addEventListener('click', () => {
  try {
    applyPasted($('paste-text').value)
    pasteDialog.close()
    notify(tr('lab_pasted'))
  } catch (error) {
    notify(tr('lab_paste_bad', { error: String(error.message ?? error) }), { error: true })
  }
})

for (const input of root.querySelectorAll('[data-overlay]')) {
  overlays[input.dataset.overlay] = input.checked
  input.addEventListener('change', () => { overlays[input.dataset.overlay] = input.checked; draw(); saveState() })
}
for (const input of root.querySelectorAll('input[name=tint]')) {
  input.addEventListener('change', () => { state.tint = input.value; draw(); renderResults(); saveState() })
}
$('level-select').addEventListener('change', (event) => { state.level = Number(event.target.value); state.selected = null; fitView(); draw(); renderResults() })

const EXPERIMENTS = {
  shapes: () => { for (const shape of CLASSES) state.P.weights[shape] = 1; state.P.m = 8 },
  cycles: () => { state.P.delta = 0.8 },
  junctions: () => { state.P.pT = Math.max(state.P.pT, 0.25); state.P.pX = Math.max(state.P.pX, 0.15) },
  rooms: () => { for (const shape of CLASSES) state.P.weights[shape] = shape === 'disc' ? 1 : 0 },
}
for (const button of root.querySelectorAll('[data-experiment]')) {
  button.addEventListener('click', () => {
    const kind = button.dataset.experiment
    remember_()
    EXPERIMENTS[kind]()
    notify(tr(`lab_exp_${kind}_done`))
    syncForm()
    changed()
  })
}

/* ---------------- Benchmarks (Developer) ---------------- */

const benchStatus = $('bench-status')
$('bench-40').addEventListener('click', () => {
  const result = JSON.parse(bench(state.size, 1, 40))
  benchStatus.textContent = tr('lab_perf_40_result', { ms: formatNumber(result.ms / 40, 2), invalid: result.invalid })
})
$('bench-800').addEventListener('click', async () => {
  benchStatus.textContent = tr('lab_perf_800_run')
  try {
    pool ??= createPool()
    const one = await pool.batch(state.size, 1, 800, 1)
    const all = await pool.batch(state.size, 1, 800)
    benchStatus.textContent = tr('lab_perf_800_result', { one: formatNumber(one.ms, 0), workers: all.workers, many: formatNumber(all.ms, 0), invalid: all.invalid })
  } catch (error) {
    benchStatus.textContent = tr('lab_perf_800_fail', { error })
  }
})

/* ---------------- Geometry of a floor ---------------- */

const canvas = $('floor-map')
const ctx = canvas.getContext('2d')
const hatches = hatchPatterns(ctx, css('--map-hatch'))
const mapTooltip = $('map-tooltip')
const zoomLabel = $('zoom-label')
const viewSection = $('view')
const progress = $('lab-progress')

const pathOf = (shape) => {
  ctx.beginPath()
  if (shape.kind === 'disc') ctx.arc(shape.c[0], shape.c[1], shape.r, 0, Math.PI * 2)
  else shape.pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
  ctx.closePath()
}
const strokePolyline = (pts) => {
  ctx.beginPath()
  pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
  ctx.stroke()
}
const boundsOfShape = (shape) => {
  if (shape.kind === 'disc') return [shape.c[0] - shape.r, shape.c[1] - shape.r, shape.c[0] + shape.r, shape.c[1] + shape.r]
  const xs = shape.pts.map((p) => p[0])
  const ys = shape.pts.map((p) => p[1])
  return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)]
}

// The level of every node: a room has its own, and a junction takes the one of the corridors that reach it.
let nodeLevels = []
let roomNode = []
let hopsOf = []
let farthest = 0
// How far each node is from the entrance, in rooms: a new floor is revealed outwards from there, like the desktop app does.
const computeHops = (d) => {
  roomNode = []
  d.nodes.forEach((node, id) => { if (node.kind === 'room') roomNode[node.room] = id })
  const origin = d.rooms.find((r) => r.role === 'entrance')?.id
  const start = origin != null ? roomNode[origin] : 0
  hopsOf = d.nodes.map(() => Infinity)
  hopsOf[start] = 0
  const queue = [start]
  while (queue.length) {
    const at = queue.shift()
    for (const edge of d.edges) {
      const next = edge.u === at ? edge.v : edge.v === at ? edge.u : null
      if (next != null && hopsOf[next] === Infinity) { hopsOf[next] = hopsOf[at] + 1; queue.push(next) }
    }
  }
  farthest = Math.max(0, ...hopsOf.filter(Number.isFinite))
}
const computeLevels = (d) => {
  computeHops(d)
  nodeLevels = d.nodes.map((node) => (node.kind === 'room' ? d.rooms[node.room].level ?? 0 : null))
  for (let round = 0; round < 3; round++) {
    for (const edge of d.edges) {
      if (edge.stair) continue
      if (nodeLevels[edge.u] == null && nodeLevels[edge.v] != null) nodeLevels[edge.u] = nodeLevels[edge.v]
      if (nodeLevels[edge.v] == null && nodeLevels[edge.u] != null) nodeLevels[edge.v] = nodeLevels[edge.u]
    }
  }
}
const levelCount = () => Math.max(1, ...current.floor.rooms.map((r) => (r.level ?? 0) + 1))
const onLevel = (room) => (room.level ?? 0) === state.level
const edgeOnLevel = (edge) => !edge.stair && (nodeLevels[edge.u] ?? 0) === state.level
const visibleRooms = () => current.floor.rooms.filter((r) => r.alive && onLevel(r))

// Fit shows the floor, with a margin, and not the empty grid around it.
let bounds = null
const computeBounds = () => {
  if (!current) return (bounds = null)
  const d = current.floor
  let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity]
  const grow = (a, b, c, e) => { x0 = Math.min(x0, a); y0 = Math.min(y0, b); x1 = Math.max(x1, c); y1 = Math.max(y1, e) }
  for (const room of d.rooms) if (room.alive && onLevel(room)) grow(...boundsOfShape(room.shape))
  for (const edge of d.edges) if (edgeOnLevel(edge)) for (const [x, y] of edge.pts) grow(x, y, x, y)
  if (!Number.isFinite(x0)) return (bounds = null)
  const pad = 0.04 * Math.max(x1 - x0, y1 - y0, 1) + d.params.width
  bounds = [x0 - pad, y0 - pad, x1 + pad, y1 + pad]
}
const transformOf = () => {
  const W = canvas.clientWidth
  const H = canvas.clientHeight
  const P = current?.floor.params
  if (!P) return { W, H, k: 1, ox: 0, oy: 0 }
  const [x0, y0, x1, y1] = bounds ?? [0, 0, P.cols * P.cellW, P.rows * P.cellH]
  const worldW = x1 - x0
  const worldH = y1 - y0
  const fit = Math.min(Math.max(1, W - 48) / worldW, Math.max(1, H - 48) / worldH)
  const k = fit * view.zoom
  return { W, H, k, ox: W / 2 + view.x - ((x0 + x1) / 2) * k, oy: H / 2 + view.y - ((y0 + y1) / 2) * k }
}
const fitView = () => { computeBounds(); view.zoom = 1; view.x = 0; view.y = 0 }

/* ---------------- Drawing ---------------- */

// The reveal of a new floor: from 0 (nothing) to 1 (all of it) over the time Settings give, outwards from the entrance.
const reveal = { start: 0, ms: 0, running: false }
const revealFront = () => {
  if (!reveal.running) return null
  const p = (performance.now() - reveal.start) / reveal.ms
  if (p >= 1) { reveal.running = false; return null }
  return p * (farthest + 1.5)
}
const nodeAlpha = (front, id) => (front == null ? 1 : Math.max(0, Math.min(1, front - (Number.isFinite(hopsOf[id]) ? hopsOf[id] : 0))))
const startReveal = () => {
  const ms = setting().revealMs ?? 0
  if (ms <= 0 || reveal.running) { reveal.running = false; return }
  reveal.start = performance.now()
  reveal.ms = ms
  reveal.running = true
  const tick = () => { draw(); if (reveal.running) requestAnimationFrame(tick) }
  requestAnimationFrame(tick)
}

const tintOf = () => (state.tint === 'shape' ? null : current?.tints.find((t) => t.id === state.tint) ?? null)

const draw = () => {
  const dpr = window.devicePixelRatio || 1
  const W = canvas.clientWidth
  const H = canvas.clientHeight
  if (!W || !H) return
  if (canvas.width !== Math.round(W * dpr) || canvas.height !== Math.round(H * dpr)) {
    canvas.width = Math.round(W * dpr)
    canvas.height = Math.round(H * dpr)
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, W, H)
  zoomLabel.textContent = `${Math.round(view.zoom * 100)}%`
  if (!current) return
  const { k, ox, oy } = transformOf()
  ctx.setTransform(dpr * k, 0, 0, dpr * k, dpr * ox, dpr * oy)
  const px = 1 / k
  const d = current.floor
  const P = d.params
  const ink = css('--ink')
  const accent = css('--accent')
  const corridor = css('--map-corridor')
  const surface = css('--control')
  const colorOf = (cls) => css(`--map-${cls}`) || '#59677a'
  const roleColor = (role) => css(`--role-${role}`)
  const tint = tintOf()
  const front = revealFront()

  if (overlays.cells) {
    ctx.strokeStyle = css('--rule')
    ctx.globalAlpha = 0.5
    ctx.lineWidth = px
    const gridX = d.gridX?.length === P.cols + 1 ? d.gridX : Array.from({ length: P.cols + 1 }, (_, i) => i * P.cellW)
    const gridY = d.gridY?.length === P.rows + 1 ? d.gridY : Array.from({ length: P.rows + 1 }, (_, i) => i * P.cellH)
    for (const x of gridX) strokePolyline([[x, gridY[0]], [x, gridY.at(-1)]])
    for (const y of gridY) strokePolyline([[gridX[0], y], [gridX.at(-1), y]])
    ctx.globalAlpha = 1
  }
  if (overlays.pairs) {
    ctx.strokeStyle = accent
    ctx.globalAlpha = 0.5
    ctx.setLineDash([7 * px, 6 * px])
    ctx.lineWidth = px
    for (const [a, b] of d.pairs) if (onLevel(d.rooms[a]) && onLevel(d.rooms[b])) strokePolyline([d.rooms[a].center, d.rooms[b].center])
    ctx.setLineDash([])
    ctx.globalAlpha = 1
  }
  if (overlays.candidates) {
    ctx.strokeStyle = accent
    ctx.globalAlpha = 0.35
    ctx.lineWidth = 2 * px
    for (const candidate of d.candidates) strokePolyline(candidate.pts)
    ctx.globalAlpha = 1
  }
  const edges = d.edges.filter(edgeOnLevel)
  const edgeAlpha = (edge) => Math.min(nodeAlpha(front, edge.u), nodeAlpha(front, edge.v))
  ctx.strokeStyle = corridor
  ctx.lineWidth = P.width
  ctx.lineCap = 'butt'
  ctx.lineJoin = 'round'
  for (const edge of edges) {
    ctx.globalAlpha = edgeAlpha(edge)
    strokePolyline(edge.pts)
  }
  ctx.strokeStyle = ink
  ctx.lineWidth = px
  for (const edge of edges) {
    ctx.globalAlpha = 0.55 * edgeAlpha(edge)
    strokePolyline(edge.pts)
  }
  ctx.globalAlpha = 1
  for (const room of d.rooms) {
    if (!room.alive || !onLevel(room)) continue
    const shown = nodeAlpha(front, roomNode[room.id])
    if (shown <= 0) continue
    pathOf(room.shape)
    ctx.fillStyle = (tint && tint.colors[room.id]) || colorOf(room.cls)
    ctx.globalAlpha = (room.leaf ? 0.62 : 0.9) * shown
    ctx.fill()
    ctx.globalAlpha = shown
    const pattern = !tint && hatches[room.cls]
    if (pattern) {
      pattern.setTransform(new DOMMatrix().scale(px))
      ctx.fillStyle = pattern
      ctx.fill()
    }
    const role = roleColor(room.role)
    ctx.strokeStyle = role || ink
    ctx.lineWidth = (role ? 3.5 : 1.5) * px
    ctx.stroke()
    ctx.globalAlpha = 1
  }
  for (const room of d.rooms) {
    if (room.alive || !onLevel(room)) continue
    pathOf(room.shape)
    ctx.strokeStyle = css('--danger')
    ctx.setLineDash([4 * px, 4 * px])
    ctx.lineWidth = px
    ctx.stroke()
    ctx.setLineDash([])
  }
  if (overlays.ports) {
    ctx.fillStyle = ink
    ctx.globalAlpha = 0.35
    for (const room of d.rooms) if (onLevel(room)) for (const port of room.ports) ctx.fillRect(port.p[0] - 1.5 * px, port.p[1] - 1.5 * px, 3 * px, 3 * px)
    ctx.globalAlpha = 1
  }
  ctx.strokeStyle = ink
  ctx.lineWidth = 2 * px
  for (const room of d.rooms) if (onLevel(room)) for (const port of room.used) {
    const direction = DIRS[port.k]
    strokePolyline([port.p, [port.p[0] + direction[0] * 14, port.p[1] + direction[1] * 14]])
  }
  d.nodes.forEach((node, id) => {
    if (node.kind !== 'junction' || (nodeLevels[id] ?? 0) !== state.level) return
    ctx.beginPath()
    ctx.arc(node.p[0], node.p[1], 5, 0, Math.PI * 2)
    ctx.fillStyle = surface
    ctx.fill()
    ctx.strokeStyle = ink
    ctx.lineWidth = px
    ctx.stroke()
  })
  // The picked room: an amber outline over its own, as the desktop app draws it.
  const picked = state.selected != null ? d.rooms[state.selected] : null
  if (picked?.alive && onLevel(picked)) {
    pathOf(picked.shape)
    ctx.strokeStyle = css('--amber')
    ctx.lineWidth = 6 * px
    ctx.stroke()
    ctx.strokeStyle = css('--bg')
    ctx.lineWidth = 1.5 * px
    ctx.stroke()
  }
  if (overlays.labels) {
    ctx.font = `500 ${11 * px}px ${css('--font-mono') || 'monospace'}`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'alphabetic'
    const halo = css('--bg')
    for (const room of d.rooms) {
      if (!room.alive || !onLevel(room)) continue
      const label = `#${room.id}`
      ctx.lineWidth = 2.2 * px
      ctx.strokeStyle = halo
      ctx.strokeText(label, room.center[0], room.center[1] + 4 * px)
      ctx.fillStyle = ink
      ctx.fillText(label, room.center[0], room.center[1] + 4 * px)
    }
  }
  // Screen-sized marks, at the same size at every zoom: the stairs, and IN and OUT chips at the corner of the entrance and the exit.
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.font = `600 10px ${css('--font-mono') || 'monospace'}`
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  for (const edge of d.edges) {
    if (!edge.stair) continue
    if ((nodeLevels[edge.u] ?? 0) !== state.level && (nodeLevels[edge.v] ?? 0) !== state.level) continue
    const [x, y] = edge.pts[0]
    ctx.beginPath()
    ctx.arc(ox + x * k, oy + y * k, 9, 0, Math.PI * 2)
    ctx.fillStyle = css('--amber')
    ctx.fill()
    ctx.fillStyle = css('--bg')
    ctx.fillText('S', ox + x * k, oy + y * k + 1)
  }
  for (const room of d.rooms) {
    if (!room.alive || !onLevel(room) || room.role === 'plain' || nodeAlpha(front, roomNode[room.id]) < 0.6) continue
    const [x0, y0] = boundsOfShape(room.shape)
    const text = room.role === 'entrance' ? 'IN' : 'OUT'
    const w = text === 'IN' ? 20 : 28
    const cx = ox + x0 * k + 3
    const cy = oy + y0 * k + 3
    ctx.fillStyle = roleColor(room.role)
    ctx.fillRect(cx, cy, w, 14)
    ctx.fillStyle = css('--bg')
    ctx.fillText(text, cx + w / 2, cy + 8)
  }
}

/* ---------------- Details while exploring the plan ---------------- */

const containsPoint = (shape, point) => {
  if (shape.kind === 'disc') return Math.hypot(point[0] - shape.c[0], point[1] - shape.c[1]) <= shape.r
  let inside = false
  const pts = shape.pts
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const [xi, yi] = pts[i]
    const [xj, yj] = pts[j]
    if ((yi > point[1]) !== (yj > point[1]) && point[0] < ((xj - xi) * (point[1] - yi)) / (yj - yi) + xi) inside = !inside
  }
  return inside
}
const distanceToSegment = (point, a, b) => {
  const dx = b[0] - a[0]
  const dy = b[1] - a[1]
  const length2 = dx * dx + dy * dy
  const t = length2 ? Math.max(0, Math.min(1, ((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / length2)) : 0
  return Math.hypot(point[0] - (a[0] + t * dx), point[1] - (a[1] + t * dy))
}
const nodeLabel = (floor, nodeId) => {
  const node = floor.nodes[nodeId]
  if (node?.kind === 'junction') return tr('lab_node_junction')
  const room = floor.rooms[node?.room]
  return room ? tr('lab_node_room', { id: room.id }) : tr('lab_node_point')
}
const roomText = (room) => {
  const role = room.leaf ? tr('lab_leaf_chamber') : roleName(room.role)
  return tr('lab_tip_room', { role, id: room.id, class: className(room.cls), area: formatNumber(room.area, 0), used: room.used.length, possible: room.ports.length })
}
const roomAt = (point) => [...current.floor.rooms].reverse().find((room) => room.alive && onLevel(room) && containsPoint(room.shape, point))

const mapObjectAt = (point, scale) => {
  const d = current.floor
  const room = roomAt(point)
  if (room) return roomText(room)
  const junction = d.nodes.find((node, id) => node.kind === 'junction' && (nodeLevels[id] ?? 0) === state.level && Math.hypot(node.p[0] - point[0], node.p[1] - point[1]) <= Math.max(6, 8 / scale))
  if (junction) {
    const id = d.nodes.indexOf(junction)
    const degree = d.edges.filter((edge) => edge.u === id || edge.v === id).length
    return tr('lab_tip_junction', { degree, x: formatNumber(junction.p[0], 0), y: formatNumber(junction.p[1], 0) })
  }
  const threshold = d.params.width / 2 + 5 / scale
  for (const edge of d.edges.filter(edgeOnLevel)) {
    for (let i = 1; i < edge.pts.length; i++) {
      if (distanceToSegment(point, edge.pts[i - 1], edge.pts[i]) > threshold) continue
      const length = edge.pts.slice(1).reduce((sum, p, j) => sum + Math.hypot(p[0] - edge.pts[j][0], p[1] - edge.pts[j][1]), 0)
      return tr('lab_tip_corridor', { id: edge.id, from: nodeLabel(d, edge.u), to: nodeLabel(d, edge.v), n: edge.pts.length - 1, length: formatNumber(length, 0) })
    }
  }
  return ''
}

const hideMapTooltip = () => { mapTooltip.hidden = true }
const worldOf = (event) => {
  const rect = canvas.getBoundingClientRect()
  const { k, ox, oy } = transformOf()
  return { point: [(event.clientX - rect.left - ox) / k, (event.clientY - rect.top - oy) / k], k }
}
const showMapTooltip = (event) => {
  if (!current) return
  const { point, k } = worldOf(event)
  const content = mapObjectAt(point, k)
  if (!content) return hideMapTooltip()
  mapTooltip.textContent = content
  mapTooltip.hidden = false
  const viewRect = viewSection.getBoundingClientRect()
  mapTooltip.style.left = `${Math.max(10, Math.min(event.clientX - viewRect.left + 14, viewRect.width - 250))}px`
  mapTooltip.style.top = `${Math.max(58, Math.min(event.clientY - viewRect.top + 14, viewRect.height - 100))}px`
}

const pick = (id) => {
  state.selected = id
  draw()
  renderSelected()
  announce(id == null ? tr('lab_released') : tr('lab_picked', { id }))
}

/* ---------------- Results panel ---------------- */

const results = $('results-body')
const selectedBox = $('selected-room')
const swatch = (color, shape) => { const i = el('i', { 'aria-hidden': 'true' }); i.style.backgroundColor = color; if (shape) i.dataset.shape = shape; return i }

const translate = (message, table) => {
  for (const [pattern, id] of table) {
    const match = typeof pattern === 'string' ? (message === pattern ? [] : null) : message.match(pattern)
    if (match) return tr(id, { a: match[1], b: match[2], c: match[3] })
  }
  return message
}
const VIOLATIONS = [
  [/^V1 room (\d+) not reached from the start$/, 'lab_v_v1'],
  [/^V2 rooms (\d+) and (\d+) closer than 2g$/, 'lab_v_v2_rooms'],
  [/^V4 corridor (.+) reaches (\d+) room\(s\): every corridor joins rooms$/, 'lab_v_v4_reach'],
  [/^V4 rooms (\d+) and (\d+): corridor (.+) px < ℓmin$/, 'lab_v_v4_short'],
  [/^V2 rooms (\d+) and (\d+) joined but closer than ℓmin in a straight line$/, 'lab_v_v2_straight'],
  [/^V2 corridor (\d+) runs into room (\d+) past its door$/, 'lab_v_v2_past_door'],
  [/^V2 corridor (\d+) closer than σ to room (\d+)$/, 'lab_v_v2_corr_room'],
  [/^V2 corridors (\d+) and (\d+) closer than σ$/, 'lab_v_v2_corr_corr'],
  [/^V3 room (\d+): a door at 45° in a retilinear stage$/, 'lab_v_v3_diag'],
  [/^V3 room (\d+): a door's exit band runs back into the room$/, 'lab_v_v3_back'],
  [/^V3 room (\d+): a door off the circle$/, 'lab_v_v3_circle'],
  [/^V3 room (\d+): doors (\d+) and (\d+) closer than σ$/, 'lab_v_v3_doors'],
  [/^V4 corridor (\d+): a segment off the alphabet$/, 'lab_v_v4_alphabet'],
  [/^V4 corridor (\d+): a (\d+)° turn$/, 'lab_v_v4_turn'],
  [/^V4 corridor (\d+): a segment of (.+) px$/, 'lab_v_v4_segment'],
  [/^V4 corridor (\d+): leaves a door at an angle$/, 'lab_v_v4_angle'],
  [/^V5 the exit has (\d+) doors \(most (\d+)\)$/, 'lab_v_v5_doors'],
  [/^V5 the exit is (.+) hops from the entrance \(least (\d+)\)$/, 'lab_v_v5_hops'],
  ['V5 the entrance and the exit must be two rooms of the stage', 'lab_v_v5_distinct'],
  ['V5 the stage has no entrance room', 'lab_v_v5_no_entrance'],
  [/^T1 profile requires (\d+) side branches; found (\d+)$/, 'lab_v_t1'],
  [/^T2 profile requires branch depth (\d+); found (\d+)$/, 'lab_v_t2'],
  [/^T3 profile requires (\d+) independent routes to the exit; found (\d+)$/, 'lab_v_t3'],
  [/^T4 profile requires a hub degree of (\d+); found (\d+)$/, 'lab_v_t4'],
  [/^T5 profile requires a cycle of at least (\d+) branches; found (\d+)$/, 'lab_v_t5'],
]
const LOGS = [
  [/^tree stuck: candidates from (.+)$/i, 'lab_l_stuck_candidates'],
  [/^tree stuck: near (.+) px$/i, 'lab_l_stuck_near'],
  [/^(.+) room\(s\) left out: no valid corridor reaches them$/, 'lab_l_left_out'],
  [/^exit only (.+) hops from the entrance \(wanted (.+)\)$/, 'lab_l_exit'],
  ['topology profile: side branches and independent routes need an entrance and an exit, so they are left out', 'lab_l_anchors'],
  ['topology profile: independent routes to the exit are not supported with several islands, so they are left out', 'lab_l_islands'],
  [/^β\* = (.+) not reached: nothing more fits \(β = (.+)\)$/, 'lab_l_beta'],
  [/^block (\d+): (.+) does not fit, rect instead$/, 'lab_l_block'],
  [/^(.+) of (.+) leaf chambers fit$/, 'lab_l_leaves'],
  [/^topology profile: could not reach (\d+) independent routes to the exit \(λ = (\d+)\)$/, 'lab_l_routes'],
  [/^topology profile: no valid junction can reach degree (\d+)$/, 'lab_l_hub'],
  [/^topology profile: no valid cycle of at least (\d+) branches fits$/, 'lab_l_cycle'],
  [/^topology profile: added (\d+) of (\d+) required side branches$/, 'lab_l_branches'],
  [/^layout jitter limited to (.+) \(requested (.+)\) to preserve minimum room size$/, 'lab_l_jitter'],
  [/shape class caps cannot fit/i, 'lab_l_caps'],
]

// The numbers behind the measures, grouped; each says what it means in text, not only on hover.
const METRIC_ROWS = (m) => ({
  lab_g_size: [
    ['rooms', `${m.rooms}`], ['leaf_chambers', `${m.leafChambers}`], ['junctions', `${m.junctions}`], ['edges', `${m.edges}`],
  ],
  lab_g_cycles: [
    ['beta', `${m.beta}`], ['beta_target', `${m.betaTarget}`], ['beta_max', `${m.betaMax}`], ['delta', formatNumber(m.delta, 2)],
  ],
  lab_g_paths: [
    ['mean_degree', formatNumber(m.meanDegree, 2)], ['leaves', `${m.leaves}`],
    ['bridges', `${m.bridges} · ${formatNumber(m.bridgeShare * 100, 0)}%`], ['articulations', `${m.articulations}`],
    ['hops', m.hopsToExit == null ? '—' : formatNumber(m.hopsToExit, 0)], ['diameter', formatNumber(m.diameter, 0)],
    ['routes', m.routesToExit == null ? '—' : `${m.routesToExit}`], ['girth', m.cycleGirth == null ? '—' : `${m.cycleGirth}`],
    ['side_branches', `${m.sideBranches}`], ['branch_depth', `${m.deepestBranch}`], ['max_degree', `${m.maxNodeDegree}`],
    ['geo_path', m.geometricPathToExit == null ? '—' : `${formatNumber(m.geometricPathToExit, 0)} px`], ['lambda2', formatNumber(m.lambda2, 3)],
  ],
})

const attempts = (n) => `${n} attempt${n === 1 ? '' : 's'}`
const roomsIn = (message) => [...message.matchAll(/rooms? (\d+)(?: and (\d+))?/g)].flatMap((m) => [m[1], m[2]]).filter(Boolean).map(Number)

const renderSelected = () => {
  selectedBox.replaceChildren()
  const room = state.selected != null ? current?.floor.rooms[state.selected] : null
  if (!room) return
  selectedBox.append(el('section', { class: 'result-section selected-card' },
    el('h3', {}, tr('lab_selected_title')),
    el('p', { class: 'selected-text' }, roomText(room)),
    el('p', { class: 'panel-meta' }, tr('lab_selected_hint', { doors: `${room.used.length}/${room.ports.length}` }))))
}

const renderRules = () => {
  const violations = current.violations
  const ok = violations.length === 0
  const rules = current.rules.map(([code, name, detail]) => {
    const mine = violations.filter((v) => v.startsWith(code))
    return el('li', { class: mine.length ? 'rule failed' : 'rule' },
      el('span', { class: 'rule-mark', 'aria-hidden': 'true' }, mine.length ? '×' : '✓'),
      el('span', { class: 'rule-name' }, `${code} ${name}`),
      el('span', { class: 'rule-detail' }, detail),
      ...(mine.length ? [el('ul', { class: 'violation-list' }, ...mine.slice(0, 8).map((line) => {
        const button = el('button', { type: 'button', class: 'violation' }, translate(line, VIOLATIONS))
        const room = roomsIn(line)[0]
        if (room != null) button.addEventListener('click', () => pick(room))
        else button.disabled = true
        return el('li', {}, button)
      })), ...(mine.length > 8 ? [el('p', { class: 'panel-meta' }, `+${mine.length - 8}`)] : [])] : []))
  })
  const others = violations.filter((v) => !current.rules.some(([code]) => v.startsWith(code)))
  return el('section', { class: `validity-card ${ok ? 'valid' : 'invalid'}`, 'aria-label': tr('lab_rules_title') },
    el('div', { class: 'validity-body' },
      el('div', { class: 'validity-head' },
        el('strong', {}, ok ? tr('lab_valid') : tr('lab_invalid', { n: violations.length })),
        el('span', { class: 'hash', title: tr('lab_hash_help') }, el('span', {}, tr('lab_hash')), ` ${current.hash}`)),
      el('p', {}, ok ? tr('lab_valid_detail', { draws: attempts(current.draws), ms: formatNumber(current.ms, 1) }) : tr('lab_invalid_detail', { draws: attempts(current.draws) })),
      el('ul', { class: 'rule-list' }, ...rules),
      ...(others.length ? [el('ul', { class: 'violation-list' }, ...others.slice(0, 8).map((line) => el('li', {}, translate(line, VIOLATIONS))))] : [])))
}

const renderMeasures = () => {
  const rows = current.readout.map((row) => el('tr', { title: row.tip },
    el('th', { scope: 'row' }, row.label, el('span', { class: 'sr-only' }, `. ${row.tip}`)),
    el('td', {}, ...(row.share == null ? [] : [el('span', { class: 'bar', 'aria-hidden': 'true' }, el('i', { style: `width:${Math.round(row.share * 100)}%` }))]), row.value)))
  return el('section', { class: 'result-section' }, el('h3', {}, tr('lab_measures_title')), el('table', { class: 'metric-table measures' }, el('tbody', {}, ...rows)))
}

const renderLegend = () => {
  const m = current
  const shapeColor = (name) => `var(--map-${name})`
  const items = [
    ...CLASSES.map((name) => [swatch(shapeColor(name), name), className(name)]),
  ]
  const marks = [
    [chip('IN', 'var(--role-entrance)'), tr('lab_legend_in')],
    [chip('OUT', 'var(--role-exit)'), tr('lab_legend_out')],
    [swatch('var(--map-corridor)'), tr('lab_legend_leaf')],
    [el('i', { class: 'dot', 'aria-hidden': 'true' }), tr('lab_legend_junction')],
    [el('i', { class: 'tick', 'aria-hidden': 'true' }), tr('lab_legend_door')],
    [el('i', { class: 'pick', 'aria-hidden': 'true' }), tr('lab_legend_picked')],
    ...(m.floor.edges.some((e) => e.stair) ? [[chip('S', 'var(--amber)'), tr('lab_legend_stairs')]] : []),
  ]
  const tint = tintOf()
  const scale = tint ? [el('div', { class: 'ramp' },
    el('span', {}, `${tint.lo}`),
    el('i', { style: `background: linear-gradient(90deg, ${tint.ramp[0]}, ${tint.ramp[1]})`, 'aria-hidden': 'true' }),
    el('span', {}, `${tint.hi}`),
    ...(tint.unit === '' ? [el('span', { class: 'panel-meta' }, `(${tr('lab_tint_relative')})`)] : []))] : []
  const line = (list) => el('div', { class: 'legend-list' }, ...list.map(([mark, name]) => el('span', { class: 'legend-item' }, mark, name)))
  return el('details', { class: 'result-group' }, el('summary', {}, tr('lab_legend_title')),
    el('div', { class: 'legend-block' }, el('strong', {}, tr('lab_legend_shapes')), line(items), el('strong', {}, tr('lab_legend_roles')), line(marks), ...scale))
}
const chip = (text, color) => { const c = el('b', { class: 'chip', 'aria-hidden': 'true' }, text); c.style.background = color; return c }

const renderResults = () => {
  results.replaceChildren()
  if (!current) return
  const m = current.metrics
  results.append(el('div', { class: 'panel-header' }, el('h2', {}, tr('lab_rules_title')), el('p', { class: 'panel-meta' }, tr('lab_panel_meta', { size: SIZE_NAMES[state.size - 1], seed: state.seed }))))
  results.append(renderRules(), renderMeasures())
  const groups = METRIC_ROWS(m)
  const tables = Object.entries(groups).map(([title, rows]) =>
    el('section', { class: 'numbers-group' }, el('h4', {}, tr(title)),
      el('table', { class: 'metric-table' }, el('tbody', {}, ...rows.map(([id, value]) =>
        el('tr', { title: tr(`lab_m_${id}_help`) }, el('th', { scope: 'row' }, tr(`lab_m_${id}`), el('span', { class: 'sr-only' }, `. ${tr(`lab_m_${id}_help`)}`)), el('td', {}, value)))))))
  results.append(el('details', { class: 'result-group' }, el('summary', {}, tr('lab_more_numbers')), ...tables))
  const chips = Object.entries(m.byClass).map(([name, count]) => el('span', { class: 'shape-chip' }, swatch(`var(--map-${name})`, name), `${className(name)}: ${count}`))
  results.append(el('section', { class: 'result-section' }, el('h3', {}, tr('lab_shapes_title')), el('div', { class: 'chip-list' }, ...chips)))
  if (current.floor.log.length) {
    results.append(el('details', { class: 'result-group' }, el('summary', {}, tr('lab_log_title')), el('ul', { class: 'log-list' }, ...current.floor.log.map((line) => el('li', {}, translate(line, LOGS))))))
  }
  results.append(renderLegend())
  renderSelected()
}

/* ---------------- Generation ---------------- */

// A floor is made by the worker. While the next one is on its way the last one stays drawn, and the reading says it is the last one.
const staleNote = el('p', { class: 'stale-note', id: 'stale-note', hidden: '' })
$('results').prepend(staleNote)
const setBusy = () => {
  root.setAttribute('aria-busy', String(pending))
  progress.textContent = pending ? tr('lab_generating', { size: SIZE_NAMES[state.size - 1] }) : ''
  staleNote.textContent = tr('lab_stale')
  staleNote.hidden = !(pending || stale) || !current
  results.classList.toggle('stale', (pending || stale) && !!current)
}
const labelCanvas = () => {
  const n = current ? visibleRooms().length : 0
  const status = !current ? 'loading' : current.violations.length ? tr('lab_state_invalid', { n: current.violations.length }) : tr('lab_state_valid')
  canvas.setAttribute('aria-label', tr('lab_map_label', { rooms: n, state: status }))
}
const setCurrent = (answer, { animate = false } = {}) => {
  current = answer
  computeLevels(current.floor)
  if (state.level >= levelCount()) state.level = 0
  fitView()
  state.selected = state.selected != null && current.floor.rooms[state.selected]?.alive ? state.selected : null
  updateTints()
  updateLevels()
  markUnused()
  labelCanvas()
  setBusy()
  if (animate) startReveal()
  draw()
  renderResults()
}
const updateTints = () => {
  const have = new Set(current.tints.map((t) => t.id))
  for (const label of root.querySelectorAll('[data-tint]')) {
    const input = label.querySelector('input')
    input.disabled = !have.has(label.dataset.tint)
    label.title = input.disabled ? tr('lab_tint_unavailable') : ''
  }
  if (state.tint !== 'shape' && !have.has(state.tint)) state.tint = 'shape'
  for (const input of root.querySelectorAll('input[name=tint]')) input.checked = input.value === state.tint
}
const updateLevels = () => {
  const n = levelCount()
  $('level-picker').hidden = n < 2
  const select = $('level-select')
  select.replaceChildren(...Array.from({ length: n }, (_, i) => el('option', { value: String(i) }, tr('lab_level_n', { n: i + 1 }))))
  select.value = String(state.level)
}
const regenerate = async () => {
  const mine = ++generation
  pending = true
  setBusy()
  try {
    const answer = JSON.parse(await stageAsync(state.seed >>> 0, JSON.stringify(state.P)))
    if (mine !== generation) return
    pending = false
    stale = false
    clearMessage('generation')
    setCurrent(answer, { animate: true })
    saveState()
  } catch (error) {
    if (mine !== generation) return
    pending = false
    stale = true
    notify(`${tr('lab_gen_failed')}: ${String(error?.message ?? error)}`, { error: true, key: 'generation' })
    setBusy()
  }
}
function scheduleRegenerate() {
  clearTimeout(regenTimer)
  updateSizeButtons()
  hideHelp()
  regenTimer = setTimeout(regenerate, 80)
}

/* ---------------- Keeping the state, and the link ---------------- */

let saveTimer = 0
function saveState() {
  clearTimeout(saveTimer)
  saveTimer = setTimeout(() => {
    remember('tapiti.lab.state', JSON.stringify({ size: state.size, seed: state.seed, diff: differences(), overlays, tint: state.tint }))
  }, 200)
}
const restoreState = () => {
  const search = new URLSearchParams(location.search)
  let saved = null
  try { saved = JSON.parse(remember('tapiti.lab.state') ?? 'null') } catch { saved = null }
  const fromUrl = search.has('size') || search.has('seed') || search.has('p')
  let size = state.size
  let seed = state.seed
  let diff = {}
  if (fromUrl) {
    const named = SIZE_NAMES.indexOf((search.get('size') ?? '').toLowerCase()) + 1
    if (named) size = named
    const asked = search.get('seed')
    const n = asked === null || asked.trim() === '' ? NaN : Number(asked)
    if (Number.isInteger(n) && n >= 0 && n <= 0xffffffff) seed = n
    try { diff = JSON.parse(fromBase64Url(search.get('p') ?? 'e30')) } catch { diff = {} }
  } else if (saved) {
    if (Number.isInteger(saved.size) && saved.size >= 1 && saved.size <= 4) size = saved.size
    if (Number.isInteger(saved.seed)) seed = saved.seed
    diff = saved.diff ?? {}
    Object.assign(overlays, saved.overlays ?? {})
    if (typeof saved.tint === 'string') state.tint = saved.tint
  }
  state.size = size
  state.seed = seed
  state.P = fullParams(size)
  state.defaults = clone(state.P)
  const names = new Set(fields.map((f) => f.name))
  for (const [path, value] of Object.entries(diff)) if (names.has(path)) setPath(state.P, path, value)
  seedInput.value = String(seed)
  for (const input of root.querySelectorAll('[data-overlay]')) input.checked = !!overlays[input.dataset.overlay]
}

/* ---------------- Pan, zoom, keys ---------------- */

const zoomBy = (factor) => {
  const next = Math.max(0.3, Math.min(20, view.zoom * factor))
  const ratio = next / view.zoom
  view.x *= ratio
  view.y *= ratio
  view.zoom = next
  draw()
}
const resetView = () => { fitView(); draw() }
const panBy = (dx, dy) => { view.x += dx; view.y += dy; draw() }
$('zoom-out').addEventListener('click', () => zoomBy(0.8))
$('zoom-in').addEventListener('click', () => zoomBy(1.25))
$('zoom-fit').addEventListener('click', resetView)

const zoomAt = (clientX, clientY, factor) => {
  const rect = canvas.getBoundingClientRect()
  const sx = clientX - rect.left - rect.width / 2
  const sy = clientY - rect.top - rect.height / 2
  const next = Math.max(0.3, Math.min(20, view.zoom * factor))
  const ratio = next / view.zoom
  view.x = sx - (sx - view.x) * ratio
  view.y = sy - (sy - view.y) * ratio
  view.zoom = next
  draw()
}

const pointers = new Map()
let drag = null
let pinch = null
canvas.addEventListener('wheel', (event) => {
  event.preventDefault()
  const dy = Math.max(-100, Math.min(100, event.deltaY * (event.deltaMode === 1 ? 33 : 1)))
  zoomAt(event.clientX, event.clientY, Math.exp(-dy * 0.0012))
}, { passive: false })
canvas.addEventListener('pointerdown', (event) => {
  hideMapTooltip()
  pointers.set(event.pointerId, { x: event.clientX, y: event.clientY })
  canvas.setPointerCapture(event.pointerId)
  if (pointers.size === 2) {
    const [a, b] = [...pointers.values()]
    pinch = { d: Math.hypot(a.x - b.x, a.y - b.y) }
    drag = null
    return
  }
  drag = { x: event.clientX, y: event.clientY, moved: false }
  canvas.classList.add('dragging')
})
canvas.addEventListener('pointermove', (event) => {
  if (pointers.has(event.pointerId)) pointers.set(event.pointerId, { x: event.clientX, y: event.clientY })
  if (pinch && pointers.size >= 2) {
    const [a, b] = [...pointers.values()]
    const d = Math.hypot(a.x - b.x, a.y - b.y)
    if (pinch.d > 0) zoomAt((a.x + b.x) / 2, (a.y + b.y) / 2, d / pinch.d)
    pinch.d = d
    return
  }
  if (!drag) return event.pointerType === 'touch' ? undefined : showMapTooltip(event)
  const dx = event.clientX - drag.x
  const dy = event.clientY - drag.y
  if (Math.abs(dx) + Math.abs(dy) > 2) drag.moved = true
  if (drag.moved) {
    view.x += dx
    view.y += dy
    drag.x = event.clientX
    drag.y = event.clientY
    hideMapTooltip()
    draw()
  }
})
const endPointer = (event) => {
  pointers.delete(event.pointerId)
  if (pointers.size < 2) pinch = null
}
canvas.addEventListener('pointerup', (event) => {
  const wasDrag = drag?.moved
  const hadPinch = pinch !== null
  endPointer(event)
  drag = null
  canvas.classList.remove('dragging')
  if (wasDrag || hadPinch || !current) return
  // A click picks the room under it, or lets go; the tooltip is for hovering.
  const { point } = worldOf(event)
  const room = roomAt(point)
  pick(room ? room.id : null)
})
canvas.addEventListener('pointercancel', (event) => { endPointer(event); drag = null; canvas.classList.remove('dragging'); hideMapTooltip() })
canvas.addEventListener('pointerleave', (event) => { if (!drag && event.pointerType !== 'touch') hideMapTooltip() })
canvas.addEventListener('dblclick', resetView)

new ResizeObserver(draw).observe(viewSection)

// The plan from the keyboard, while it has the focus: arrows move it, + and - zoom, 0 or Home fits it, [ and ] pick the previous and the
// next room, Enter picks the room in the middle, Esc lets go.
canvas.addEventListener('keydown', (event) => {
  if (event.altKey || event.ctrlKey || event.metaKey || !current) return
  const step = 60
  const rooms = visibleRooms()
  const at = rooms.findIndex((r) => r.id === state.selected)
  const handled = {
    ArrowLeft: () => panBy(step, 0),
    ArrowRight: () => panBy(-step, 0),
    ArrowUp: () => panBy(0, step),
    ArrowDown: () => panBy(0, -step),
    '+': () => zoomBy(1.25),
    '=': () => zoomBy(1.25),
    '-': () => zoomBy(0.8),
    0: resetView,
    Home: resetView,
    Escape: () => pick(null),
    ']': () => rooms.length && pick(rooms[(at + 1) % rooms.length].id),
    '[': () => rooms.length && pick(rooms[(at - 1 + rooms.length) % rooms.length].id),
    Enter: () => {
      const { k, ox, oy } = transformOf()
      const middle = [(canvas.clientWidth / 2 - ox) / k, (canvas.clientHeight / 2 - oy) / k]
      const inside = roomAt(middle)
      const nearest = inside ?? rooms.map((r) => [Math.hypot(r.center[0] - middle[0], r.center[1] - middle[1]), r]).sort((a, b) => a[0] - b[0])[0]?.[1]
      if (nearest) pick(nearest.id)
    },
  }[event.key]
  if (!handled) return
  event.preventDefault()
  event.stopPropagation()
  handled()
})

const inField = (target) => target instanceof HTMLInputElement || target instanceof HTMLSelectElement || target instanceof HTMLTextAreaElement || target?.isContentEditable
window.addEventListener('keydown', (event) => {
  if (event.defaultPrevented || event.target === canvas && !event.altKey && !event.ctrlKey && !event.metaKey) return
  const keys = setting().shortcuts !== false
  const mod = event.ctrlKey || event.metaKey
  // Undo and redo are the text field's own while one has the keyboard, and Ctrl+Z is the browser's in a dialog.
  if (mod && !event.altKey && !inField(event.target) && keys) {
    const key = event.key.toLowerCase()
    if (key === 'z' && !event.shiftKey) { event.preventDefault(); return undo() }
    if (key === 'y' || (key === 'z' && event.shiftKey)) { event.preventDefault(); return redo() }
  }
  if (!keys || mod) return
  if (event.altKey) {
    if (event.key >= '1' && event.key <= '4') { event.preventDefault(); setSize(Number(event.key)) }
    else if (event.key.toLowerCase() === 'r') { event.preventDefault(); newSeed() }
    else if (event.key === 'ArrowLeft') { event.preventDefault(); seedStep(-1) }
    else if (event.key === 'ArrowRight') { event.preventDefault(); seedStep(1) }
    return
  }
  if (inField(event.target)) return
  if (event.key === '/') { event.preventDefault(); filterInput.focus() }
  else if (event.key === '?') { event.preventDefault(); $('keys-dialog').showModal() }
})

/* ---------------- The reading drawer and the tabs ---------------- */

const readingToggle = $('reading-toggle')
const results3 = $('results')
const setReading = (open, { focus = true } = {}) => {
  root.dataset.reading = open ? 'open' : ''
  readingToggle.setAttribute('aria-expanded', String(open))
  remember('tapiti.lab.reading', open ? '1' : '0')
  if (!focus) return
  if (open) { results3.tabIndex = -1; results3.focus() } else readingToggle.focus()
}
readingToggle.addEventListener('click', () => setReading(root.dataset.reading !== 'open'))
$('reading-close').addEventListener('click', () => setReading(false))
root.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && root.dataset.reading === 'open' && event.target !== canvas) setReading(false)
})
if (remember('tapiti.lab.reading') === '1') setReading(true, { focus: false })

const tabs = [...root.querySelectorAll('.lab-tabs [role=tab]')]
const selectTab = (tab, { focus = false } = {}) => {
  root.dataset.tab = tab.dataset.tab
  for (const other of tabs) { other.setAttribute('aria-selected', String(other === tab)); other.tabIndex = other === tab ? 0 : -1 }
  if (focus) tab.focus()
}
tabs.forEach((tab, i) => {
  tab.addEventListener('click', () => selectTab(tab))
  tab.addEventListener('keydown', (event) => {
    const next = { ArrowRight: 1, ArrowLeft: -1 }[event.key]
    if (next) { event.preventDefault(); event.stopPropagation(); selectTab(tabs[(i + next + tabs.length) % tabs.length], { focus: true }) }
  })
})
selectTab(tabs[0])

/* ---------------- Settings of the site that the lab reads ---------------- */

const applySettings = () => {
  const s = setting()
  root.style.setProperty('--reading-w', `${s.readingWidth ?? 328}px`)
  root.dataset.readingMode = s.reading ?? 'automatic'
  if (s.reading === 'shown' && root.dataset.reading !== 'open' && matchMedia('(max-width: 1279px)').matches) setReading(true, { focus: false })
  if (s.reading === 'hidden' && root.dataset.reading === 'open') setReading(false, { focus: false })
  if (setting().memoryRooms === 0) { history.previous && (history.previous.floor = null); history.redo && (history.redo.floor = null) }
}

/* ---------------- Start ---------------- */

restoreState()
syncForm()
updateSizeButtons()
applySettings()
updateHistoryButtons()
root.setAttribute('aria-busy', 'false')
await regenerate()

// The plan is drawn from the page's colours, so it is drawn again when the theme changes (the settings dialog, or the system's scheme).
addEventListener('themechange', dropColours)
addEventListener('settingschange', () => { dropColours(); applySettings() })
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', dropColours)
