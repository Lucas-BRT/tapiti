// The walk editor (open the landing page with ?edit): change the seed and size, drag the points of the camera's route, add points by
// clicking the route, delete one by double-clicking it, drag the seven stops along the route and set how long the walk rests at each.
// "Preview" walks your draft on the real page (?draft); "Copy JSON" / "Download" give walk/route.json, which the page uses from then on.
// The route is a polyline of control points: build.js rounds its bends the same way for the walk and for this preview.

const NS = 'http://www.w3.org/2000/svg'
const round = (n) => Math.round(n * 10) / 10
const DRAFT = 'tapiti.walk.draft'

export function open({ wasm, build, seed, size, route }) {
  const html = document.documentElement
  html.classList.add('walk-editing')
  let draft = { seed, size, waypoints: route?.waypoints ?? null, stations: route?.stations ?? null }
  let floor
  let built

  const el = document.createElement('div')
  el.className = 'walk-editor'
  el.innerHTML = `
    <style>
      html.walk-editing { overflow: hidden; }
      .walk-editor { position: fixed; inset: 0; z-index: 100; display: grid; grid-template-columns: minmax(0, 1fr) 340px; background: var(--abyss); color: var(--bone); font: 15px var(--font-body); }
      .walk-editor svg { width: 100%; height: 100%; touch-action: none; cursor: crosshair; }
      .walk-editor aside { overflow: auto; padding: 16px; border-left: 1px solid var(--stone-500); background: var(--sign-bg); display: grid; gap: 12px; align-content: start; }
      .walk-editor h2 { margin: 0; font-size: 1.4rem; }
      .walk-editor label { display: grid; gap: 2px; font-size: .85rem; }
      .walk-editor input, .walk-editor select, .walk-editor textarea, .walk-editor button { font: inherit; padding: 4px 8px; border: 1px solid var(--stone-500); background: #fff; color: inherit; border-radius: 4px; }
      .walk-editor button { cursor: pointer; }
      .walk-editor button.primary { background: var(--carrot); border-color: var(--carrot); font-weight: 700; }
      .walk-editor .row { display: flex; gap: 8px; align-items: end; flex-wrap: wrap; }
      .walk-editor .stop { display: grid; grid-template-columns: 2.2rem 1fr 3.4rem; gap: 8px; align-items: center; }
      .walk-editor textarea { width: 100%; height: 150px; font: 12px var(--font-mono); box-sizing: border-box; }
      .walk-editor small { color: var(--stone-500); }
      @media (max-width: 800px) { .walk-editor { grid-template-columns: 1fr; grid-template-rows: 50% 50%; } }
    </style>
    <svg role="img" aria-label="The floor from above, with the route of the walk"></svg>
    <aside>
      <h2>Walk editor</h2>
      <div class="row">
        <label>Seed <input type="number" name="seed" min="0" step="1" style="width: 8rem"></label>
        <label>Size <select name="size"><option>0</option><option>1</option><option>2</option><option>3</option></select></label>
        <button type="button" name="regen">New floor</button>
      </div>
      <small>Drag a white point to move it. Click the route to add a point; double-click a point to delete it. Drag a numbered stop along the route.</small>
      <div class="stops"></div>
      <div class="row">
        <button type="button" name="preview" class="primary">Preview walk</button>
        <button type="button" name="reset">Reset route</button>
        <button type="button" name="close">Close</button>
      </div>
      <label>route.json <textarea name="json" spellcheck="false"></textarea></label>
      <div class="row">
        <button type="button" name="copy">Copy JSON</button>
        <button type="button" name="download">Download</button>
        <button type="button" name="load">Load JSON</button>
      </div>
      <small class="status" role="status"></small>
    </aside>`
  document.body.append(el)
  const $ = (name) => el.querySelector(`[name="${name}"]`)
  const svg = el.querySelector('svg')
  const status = el.querySelector('.status')

  /** Where on the generated route a station sits: the path point at fraction u. */
  const pointAt = (u) => {
    const { points, lengths, total } = built.path
    const d = u * total
    let i = 1
    while (i < lengths.length - 1 && lengths[i] < d) i++
    const t = (d - lengths[i - 1]) / (lengths[i] - lengths[i - 1] || 1)
    return [points[(i - 1) * 3] + (points[i * 3] - points[(i - 1) * 3]) * t, points[(i - 1) * 3 + 2] + (points[i * 3 + 2] - points[(i - 1) * 3 + 2]) * t]
  }

  const refresh = () => {
    floor = floor ?? JSON.parse(wasm.stage_json(draft.seed, wasm.rich_params(draft.size))).floor
    if (!draft.waypoints) {
      // First time on this floor: start from the generated walk.
      const generated = build.build(floor)
      draft.waypoints = generated.waypoints.map(([x, z]) => [round(x), round(z)])
      draft.stations = null
      built = build.build(floor, draft)
      draft.stations = built.stations.map((s) => ({ x: round(s.x), z: round(s.y), rest: s.rest }))
    }
    built = build.build(floor, draft)
    // Stops stay on the route: after an edit each one moves to the nearest point of the new route.
    draft.stations = built.stations.map((s, i) => {
      const [x, z] = pointAt(s.u)
      return { x: round(x), z: round(z), rest: draft.stations?.[i]?.rest ?? 1.3 }
    })
    built = build.build(floor, draft)
    draw()
    panel()
  }

  // One point or stop per line, so the file reads well in a diff.
  const exported = () => {
    const lines = (list) => list.map((item) => `  ${JSON.stringify(item)}`).join(',\n')
    const stops = draft.stations.map((s, i) => ({ room: built.stations[i].room, ...s }))
    return `{\n"seed": ${draft.seed}, "size": ${draft.size},\n"waypoints": [\n${lines(draft.waypoints)}\n],\n"stations": [\n${lines(stops)}\n]\n}\n`
  }

  const make = (tag, attrs, parent = svg) => {
    const node = document.createElementNS(NS, tag)
    for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, v)
    parent.append(node)
    return node
  }

  const draw = () => {
    svg.replaceChildren()
    const [x0, y0, x1, y1] = built.bounds
    svg.setAttribute('viewBox', `${x0 - 40} ${y0 - 40} ${x1 - x0 + 80} ${y1 - y0 + 80}`)
    for (const edge of floor.edges) make('polyline', { points: edge.pts.map((p) => p.join(',')).join(' '), fill: 'none', stroke: '#dcd5c4', 'stroke-width': floor.params.width, 'stroke-linejoin': 'round', 'stroke-linecap': 'round' })
    for (const room of floor.rooms) {
      if (!room.alive) continue
      const s = room.shape
      const style = { fill: '#ece6d6', stroke: '#8a8478', 'stroke-width': 2 }
      if (s.kind === 'disc') make('circle', { cx: s.c[0], cy: s.c[1], r: s.r, ...style })
      else make('polygon', { points: s.pts.map((p) => p.join(',')).join(' '), ...style })
      make('text', { x: room.center[0], y: room.center[1] + 6, 'text-anchor': 'middle', 'font-size': 18, fill: '#a39d8e' }).textContent = room.id
    }
    const p = built.path.points
    let d = ''
    for (let i = 0; i < p.length; i += 3) d += `${i ? 'L' : 'M'}${p[i]},${p[i + 2]}`
    make('path', { d, fill: 'none', stroke: '#b14c68', 'stroke-width': 5, 'stroke-linejoin': 'round' })
    draft.waypoints.forEach(([x, z], i) => make('circle', { cx: x, cy: z, r: i === 0 ? 11 : 8, fill: i === 0 ? '#b14c68' : '#fff', stroke: '#b14c68', 'stroke-width': 3, 'data-point': i, style: 'cursor: grab' }))
    draft.stations.forEach((s, i) => {
      const g = make('g', { 'data-stop': i, style: 'cursor: grab' })
      make('circle', { cx: s.x, cy: s.z, r: 20, fill: '#302e2b' }, g)
      make('text', { x: s.x, y: s.z + 7, 'text-anchor': 'middle', 'font-size': 20, fill: '#fff', 'pointer-events': 'none' }, g).textContent = i + 1
    })
    $('json').value = exported()
  }

  const panel = () => {
    $('seed').value = draft.seed
    $('size').value = draft.size
    const list = el.querySelector('.stops')
    list.replaceChildren()
    draft.stations.forEach((s, i) => {
      const row = document.createElement('label')
      row.className = 'stop'
      row.innerHTML = `<b>${i + 1}</b><input type="range" min="0.4" max="4" step="0.1" value="${s.rest}" aria-label="Rest at stop ${i + 1}, in screen heights"><output>${s.rest.toFixed(1)}</output>`
      row.querySelector('input').addEventListener('input', (event) => {
        s.rest = Number(event.target.value)
        row.querySelector('output').textContent = s.rest.toFixed(1)
        $('json').value = exported()
      })
      list.append(row)
    })
    list.prepend(Object.assign(document.createElement('small'), { textContent: 'Rest at each stop (screen heights of scrolling):' }))
  }

  /* Pointer: drag a point or a stop; click the route to add a point. */
  const at = (event) => {
    const m = svg.getScreenCTM().inverse()
    const pt = new DOMPoint(event.clientX, event.clientY).matrixTransform(m)
    return [pt.x, pt.y]
  }
  let drag = null
  svg.addEventListener('pointerdown', (event) => {
    const point = event.target.closest('[data-point]')
    const stop = event.target.closest('[data-stop]')
    if (point) drag = { point: Number(point.dataset.point) }
    else if (stop) drag = { stop: Number(stop.dataset.stop) }
    else {
      // Insert into the nearest segment of the control polyline when the click is near it.
      const [x, z] = at(event)
      let best = 30
      let index = -1
      const w = draft.waypoints
      for (let i = 1; i < w.length; i++) {
        const [ax, az] = w[i - 1]
        const [bx, bz] = w[i]
        const len2 = (bx - ax) ** 2 + (bz - az) ** 2 || 1
        const t = Math.max(0, Math.min(1, ((x - ax) * (bx - ax) + (z - az) * (bz - az)) / len2))
        const dist = Math.hypot(ax + t * (bx - ax) - x, az + t * (bz - az) - z)
        if (dist < best) { best = dist; index = i }
      }
      if (index < 0) return
      w.splice(index, 0, [round(x), round(z)])
      drag = { point: index }
      refresh()
    }
    svg.setPointerCapture(event.pointerId)
  })
  svg.addEventListener('pointermove', (event) => {
    if (!drag) return
    const [x, z] = at(event)
    if (drag.point !== undefined) draft.waypoints[drag.point] = [round(x), round(z)]
    else draft.stations[drag.stop] = { ...draft.stations[drag.stop], x: round(x), z: round(z) }
    refresh()
  })
  svg.addEventListener('pointerup', () => { drag = null })
  svg.addEventListener('dblclick', (event) => {
    const point = event.target.closest('[data-point]')
    if (!point || draft.waypoints.length <= 2 || Number(point.dataset.point) === 0) return
    draft.waypoints.splice(Number(point.dataset.point), 1)
    refresh()
  })

  /* Buttons. */
  const say = (text) => { status.textContent = text }
  const reload = () => { floor = null; draft.waypoints = null; draft.stations = null; refresh() }
  $('regen').addEventListener('click', () => { draft.seed = Number($('seed').value) || 0; draft.size = Number($('size').value); reload() })
  $('reset').addEventListener('click', reload)
  $('copy').addEventListener('click', () => navigator.clipboard.writeText(exported()).then(() => say('Copied. Paste it to Claude.'), () => say('Could not copy: select the text and copy it.')))
  $('download').addEventListener('click', () => {
    const a = Object.assign(document.createElement('a'), { href: URL.createObjectURL(new Blob([exported()], { type: 'application/json' })), download: 'route.json' })
    a.click()
    URL.revokeObjectURL(a.href)
  })
  $('load').addEventListener('click', () => {
    try {
      const next = JSON.parse($('json').value)
      if (!Array.isArray(next.waypoints) || next.waypoints.length < 2) throw new Error('no waypoints')
      draft = { seed: next.seed, size: next.size, waypoints: next.waypoints, stations: next.stations }
      floor = null
      refresh()
      say('Loaded.')
    } catch (error) {
      say(`Not valid: ${error.message}`)
    }
  })
  $('preview').addEventListener('click', () => {
    try { localStorage.setItem(DRAFT, exported()) } catch { return say('The browser would not keep the draft.') }
    location.href = `${location.pathname}?draft=1`
  })
  $('close').addEventListener('click', () => { location.href = location.pathname })

  refresh()
}
