// The lab orbit view and leaf chamber, drawn with the same paper materials as the landing walk. Scrolling carries a camera along the main path of one real floor, the one the
// generator draws for the seed and size in `data-seed` and `data-size`, built here in the browser by the same wasm module the lab uses.
// Scrolling stays native: this file only reads the scroll position. Without WebGL2, scripts or consent to data, the page keeps the
// server-rendered SVG of the same floor behind it. Geometry comes from build.js (pure data); this file owns everything that touches the GPU.

const root = document.querySelector('.walk-backdrop[data-three]')
const sceneMode = root?.dataset.mode ?? 'landing'

const params = new URLSearchParams(location.search)
const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches || params.get('motion') === 'reduce'
const saveData = () => navigator.connection?.saveData === true

/** Colours come from the page's own tokens, so CSS stays the one place that says what the stone and the torches look like. */
const css = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim()
const clamp = (x, lo, hi) => (x < lo ? lo : x > hi ? hi : x)
const smooth = (a, b, x) => { const t = clamp((x - a) / (b - a), 0, 1); return t * t * (3 - 2 * t) }
const lerp = (a, b, t) => a + (b - a) * t
const centreOfBounds = ([x0, y0, x1, y1]) => [(x0 + x1) / 2, (y0 + y1) / 2]

function hasWebGL2() {
  try {
    return !!document.createElement('canvas').getContext('webgl2')
  } catch {
    return false
  }
}

function remember(key, value, store = localStorage) {
  try {
    if (value === undefined) return store.getItem(key)
    store.setItem(key, value)
  } catch {
    return null
  }
}

/* ---------------- The floor texture ---------------- */

/** One canvas: flagstone joints along the floor's grid, each room tinted toward its shape colour, the mask as alpha. */
function floorTexture(THREE, paper, floor, scene, size) {
  const [x0, y0, x1, y1] = scene.bounds
  const W = size
  const H = Math.round((size * (y1 - y0)) / (x1 - x0))
  const sx = W / (x1 - x0)
  const sy = H / (y1 - y0)
  const canvas = document.createElement('canvas')
  canvas.width = W
  canvas.height = H
  const c = canvas.getContext('2d')
  const m = scene.mask
  const small = document.createElement('canvas')
  small.width = m.nx
  small.height = m.ny
  const image = small.getContext('2d').createImageData(m.nx, m.ny)
  for (let i = 0; i < m.data.length; i++) { image.data[i * 4] = image.data[i * 4 + 1] = image.data[i * 4 + 2] = 255; image.data[i * 4 + 3] = m.data[i] }
  small.getContext('2d').putImageData(image, 0, 0)
  c.imageSmoothingEnabled = true
  c.drawImage(small, (m.ox - x0 + m.cell / 2) * sx, (m.oy - y0 + m.cell / 2) * sy, m.nx * m.cell * sx, m.ny * m.cell * sy)
  c.globalCompositeOperation = 'source-in'
  c.fillStyle = css('--scene-floor')
  c.fillRect(0, 0, W, H)
  c.globalCompositeOperation = 'source-atop'
  const hatches = paper.hatchPatterns(c, css('--map-hatch'))
  for (const room of floor.rooms) {
    if (!room.alive) continue
    const s = room.shape
    c.beginPath()
    if (s.kind === 'disc') c.arc((s.c[0] - x0) * sx, (s.c[1] - y0) * sy, s.r * sx, 0, Math.PI * 2)
    else s.pts.forEach(([x, y], i) => (i ? c.lineTo((x - x0) * sx, (y - y0) * sy) : c.moveTo((x - x0) * sx, (y - y0) * sy)))
    c.closePath()
    c.globalAlpha = 0.75
    c.fillStyle = css(`--map-${room.cls}`) || '#888'
    c.fill()
    const pattern = hatches[room.cls]
    if (pattern) {
      c.globalAlpha = 1
      pattern.setTransform(new DOMMatrix().scale(sx, sy))
      c.fillStyle = pattern
      c.fill()
    }
  }
  c.globalAlpha = 0.2
  c.strokeStyle = css('--scene-ink')
  c.lineWidth = 1
  const joints = (list, vertical) => {
    for (let i = 0; i < list.length; i++) {
      const lo = list[i]
      const hi = list[i + 1] ?? lo
      for (const at of [lo, (lo + hi) / 2]) {
        c.beginPath()
        if (vertical) { c.moveTo((at - x0) * sx, 0); c.lineTo((at - x0) * sx, H) } else { c.moveTo(0, (at - y0) * sy); c.lineTo(W, (at - y0) * sy) }
        c.stroke()
      }
    }
  }
  joints(floor.gridX, true)
  joints(floor.gridY, false)
  c.globalAlpha = 1
  const texture = new THREE.CanvasTexture(canvas)
  texture.colorSpace = THREE.SRGBColorSpace
  texture.anisotropy = 4
  return texture
}

/* ---------------- Mount ---------------- */

/**
 * Builds the scene for `floor` on `canvas` and returns a handle. `mode` is 'landing' (scroll-driven), later 'leaf' and 'orbit'.
 * `three` is the vendored module, `scene` the result of build.js.
 */
export function mount({ THREE, paper, canvas, floor, scene: built, mode = 'landing', page }) {
  const abyss = new THREE.Color(css('--abyss'))
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: devicePixelRatio < 1.5, powerPreference: 'low-power' })
  renderer.outputColorSpace = THREE.SRGBColorSpace
  renderer.setClearColor(abyss, 1)

  const scene = new THREE.Scene()
  scene.background = abyss
  scene.fog = new THREE.FogExp2(abyss, 0.0016)
  const camera = new THREE.PerspectiveCamera(50, 1, 10, 4000)
  let controls = null

  /* Walls */
  const wallGeometry = new THREE.BufferGeometry()
  const w = built.walls
  wallGeometry.setAttribute('position', new THREE.BufferAttribute(w.positions, 3))
  wallGeometry.setAttribute('normal', new THREE.BufferAttribute(w.normals, 3))
  wallGeometry.setAttribute('uv', new THREE.BufferAttribute(w.uvs, 2))
  wallGeometry.setIndex(new THREE.BufferAttribute(w.indices, 1))
  const wallMaterial = paper.wallMaterial(THREE, css('--scene-wall'), css('--scene-ink'))
  scene.add(new THREE.Mesh(wallGeometry, wallMaterial))

  /* Floor: one plane under everything, the texture's alpha cuts it to the shape of the floor. */
  const [bx0, by0, bx1, by1] = built.bounds
  const texture = floorTexture(THREE, paper, floor, built, 1024)
  const plane = new THREE.PlaneGeometry(bx1 - bx0, by1 - by0)
  const floorMaterial = new THREE.MeshBasicMaterial({ map: texture, alphaTest: 0.5 })
  const ground = new THREE.Mesh(plane, floorMaterial)
  ground.rotation.x = -Math.PI / 2
  ground.position.set((bx0 + bx1) / 2, 0, (by0 + by1) / 2)
  scene.add(ground)

  /* Corner pillars at the junctions hide the seams where corridors cross. */
  const pillars = built.pillars.length / 2
  let pillarMesh = null
  if (pillars) {
    const geometry = new THREE.BoxGeometry(7, 60, 7)
    const uv = geometry.attributes.uv
    for (let i = 0; i < uv.count; i++) uv.setXY(i, uv.getX(i) * 7, uv.getY(i) * 60)
    pillarMesh = new THREE.InstancedMesh(geometry, paper.wallMaterial(THREE, css('--scene-wall'), css('--scene-ink')), pillars)
    const m = new THREE.Matrix4()
    for (let i = 0; i < pillars; i++) pillarMesh.setMatrixAt(i, m.makeTranslation(built.pillars[i * 2], 30, built.pillars[i * 2 + 1]))
    scene.add(pillarMesh)
  }

  /* The stairwell: a square cut into the floor, steps going down, and cold light rising from it. */
  const stairs = built.stairs
  const landingMode = mode === 'landing'
  const leafMode = mode === 'leaf'
  const orbitMode = mode === 'orbit'
  const side = Math.min(stairs.radius * 1.1, 110)
  const stepMaterial = new THREE.MeshBasicMaterial({ color: css('--scene-step') })
  const steps = []
  for (let k = 0; landingMode && k < 8; k++) {
    const step = new THREE.Mesh(new THREE.BoxGeometry(side, 20, side / 8), stepMaterial)
    step.position.set(stairs.center[0], -10 - k * 20, stairs.center[1] - side / 2 + (k + 0.5) * (side / 8))
    steps.push(step)
    scene.add(step)
  }
  /* Still, washed pencil flames. */
  const count = built.torches.length / 4
  const brackets = new THREE.InstancedMesh(new THREE.BoxGeometry(4, 16, 4), new THREE.MeshBasicMaterial({ color: css('--scene-ink') }), count)
  const flames = new THREE.InstancedMesh(new THREE.PlaneGeometry(1, 1), null, count)
  const lit = new Float32Array(count)
  const m4 = new THREE.Matrix4()
  const spots = []
  for (let i = 0; i < count; i++) {
    const x = built.torches[i * 4]
    const z = built.torches[i * 4 + 1]
    spots.push([x, 44, z])
    brackets.setMatrixAt(i, m4.makeTranslation(x, 34, z))
    flames.setMatrixAt(i, m4.makeTranslation(x, 44, z))
  }
  const litAttribute = new THREE.InstancedBufferAttribute(lit, 1)
  flames.geometry.setAttribute('lit', litAttribute)
  const flameUniforms = { uColor: { value: new THREE.Color(css('--light-torch')) }, uCore: { value: new THREE.Color('#f1d58c') }, uInk: { value: new THREE.Color(css('--scene-ink')) } }
  flames.material = new THREE.ShaderMaterial({ vertexShader: paper.FLAME_VERTEX, fragmentShader: paper.FLAME_FRAGMENT, uniforms: flameUniforms, transparent: true, depthWrite: false })
  flames.frustumCulled = false
  scene.add(brackets, flames)
  // In the leaf chamber there is one torch, brought from the nearest wall into the room.
  const leafCenter = built.leafRoom ? built.leafRoom.center : centreOfBounds(built.bounds)
  let leafTorch = -1
  if (leafMode && count) {
    let best = Infinity
    spots.forEach(([x, , z], i) => { const d = (x - leafCenter[0]) ** 2 + (z - leafCenter[1]) ** 2; if (d < best) { best = d; leafTorch = i } })
    spots[leafTorch] = [leafCenter[0] + 42, 44, leafCenter[1] - 34]
    brackets.setMatrixAt(leafTorch, m4.makeTranslation(spots[leafTorch][0], 34, spots[leafTorch][2]))
    flames.setMatrixAt(leafTorch, m4.makeTranslation(spots[leafTorch][0], 44, spots[leafTorch][2]))
  }
  /* The path */
  const pts = built.path.points
  const lens = built.path.lengths
  const total = built.path.total
  const pointAt = (u, out) => {
    const d = clamp(u, 0, 1) * total
    let lo = 0
    let hi = lens.length - 1
    while (hi - lo > 1) { const mid = (lo + hi) >> 1; if (lens[mid] <= d) lo = mid; else hi = mid }
    const span = lens[hi] - lens[lo] || 1
    const t = clamp((d - lens[lo]) / span, 0, 1)
    out[0] = lerp(pts[lo * 3], pts[hi * 3], t)
    out[2] = lerp(pts[lo * 3 + 2], pts[hi * 3 + 2], t)
    return out
  }
  const stationU = built.stations.map((s) => s.u)
  const ALTITUDE = [900, 380, 320, 290, 260, 240, 220]
  const altitude = (u) => {
    for (let i = 0; i < stationU.length - 1; i++) if (u <= stationU[i + 1]) return lerp(ALTITUDE[i], ALTITUDE[i + 1], smooth(stationU[i], stationU[i + 1], u))
    return ALTITUDE[ALTITUDE.length - 1]
  }
  const centre = [(bx0 + bx1) / 2, (by0 + by1) / 2]
  // Where each room's text stands, as a share of the width to move the picture away from it (+ right, - left). Matches landing.css.
  const SHIFT = [0.14, 0.16, 0, 0, -0.14, 0.16, -0.05]
  const shiftAt = (u) => {
    for (let i = 0; i < stationU.length - 1; i++) if (u <= stationU[i + 1]) return lerp(SHIFT[i], SHIFT[i + 1], smooth(stationU[i], stationU[i + 1], u))
    return SHIFT[SHIFT.length - 1]
  }
  const leafAngle = (t) => 0.5 + (stepped ? 0 : 0.14 * Math.sin(t * 0.35))
  const aheadU = 0.035

  /* The camera pose for walk position `u` and footer descent `dive` (0 to 1). */
  const here = [0, 0, 0]
  const ahead = [0, 0, 0]
  const pose = { px: 0, py: 0, pz: 0, lx: 0, ly: 0, lz: 0, bird: 1 }
  if (orbitMode) {
    // The lab's 3D view: turn the whole floor with the pointer or the arrow keys (when the view has the focus).
    const span = Math.max(bx1 - bx0, by1 - by0)
    controls = new THREE.OrbitControls(camera, canvas)
    controls.enableDamping = true
    controls.dampingFactor = 0.1
    controls.maxPolarAngle = Math.PI * 0.49
    controls.minDistance = 120
    controls.maxDistance = 2800
    controls.target.set(centre[0], 0, centre[1])
    camera.position.set(centre[0], span * 0.95, centre[1] + span * 0.62)
    controls.listenToKeyEvents(canvas)
    controls.update()
  }
  const poseOrbit = () => {
    pose.px = camera.position.x
    pose.py = camera.position.y
    pose.pz = camera.position.z
    pose.lx = controls.target.x
    pose.ly = controls.target.y
    pose.lz = controls.target.z
    pose.bird = 0
    here[0] = pose.lx
    here[2] = pose.lz
    return pose
  }
  const poseLeaf = (t) => {
    const a = leafAngle(t)
    pose.px = leafCenter[0] + Math.sin(a) * 175
    pose.py = 105
    pose.pz = leafCenter[1] + Math.cos(a) * 175
    pose.lx = leafCenter[0]
    pose.ly = 18
    pose.lz = leafCenter[1]
    pose.bird = 0
    here[0] = leafCenter[0]
    here[2] = leafCenter[1]
    return pose
  }
  const poseAt = (u, dive) => {
    pointAt(u, here)
    pointAt(Math.min(1, u + aheadU), ahead)
    const alt = altitude(u)
    const bird = 1 - smooth(0, stationU[1], u)
    const portrait = innerWidth < innerHeight * 0.8
    // Walking pose: above the path and a little behind, looking ahead along it.
    const back = alt * 0.7
    const wx = here[0]
    const wy = alt
    const wz = here[2] + back
    // Bird's-eye pose over the middle of the floor, so the floor reads as a map.
    const span = Math.max(bx1 - bx0, by1 - by0)
    const bxp = centre[0]
    const byp = span * (portrait ? 1.45 : 0.98)
    const bzp = centre[1] + span * 0.5
    pose.px = lerp(wx, bxp, bird)
    pose.py = lerp(wy, byp, bird)
    pose.pz = lerp(wz, bzp, bird)
    pose.lx = lerp(ahead[0], centre[0], bird)
    pose.ly = 0
    pose.lz = lerp(ahead[2], centre[1], bird)
    pose.bird = bird
    if (dive > 0) {
      // Down the stairwell and into the dark.
      const sx = stairs.center[0]
      const sz = stairs.center[1]
      pose.px = lerp(pose.px, sx, dive)
      pose.pz = lerp(pose.pz, sz + 30 * (1 - dive), dive)
      pose.py = lerp(pose.py, -70, dive)
      pose.lx = lerp(pose.lx, sx, dive)
      pose.lz = lerp(pose.lz, sz, dive)
      pose.ly = lerp(0, -260, dive)
    }
    return pose
  }

  /* Scroll to walk position */
  const sections = page ? [...page.querySelectorAll('[data-station]')] : []
  const footer = document.querySelector('.site-footer')
  let tops = []
  let heights = []
  const measure = () => {
    const base = scrollY
    tops = sections.map((s) => s.getBoundingClientRect().top + base)
    heights = sections.map((s) => s.getBoundingClientRect().height)
  }
  const target = () => {
    if (!sections.length) return { u: 0, dive: 0, station: 0 }
    const vh = innerHeight
    const mid = scrollY + vh * 0.5
    let i = 0
    for (let k = 0; k < sections.length; k++) if (tops[k] <= mid) i = k
    const f = clamp((mid - tops[i]) / (heights[i] || 1), 0, 1)
    const u = i >= stationU.length - 1 ? 1 : lerp(stationU[i], stationU[i + 1], smooth(0.35, 1, f))
    let dive = 0
    if (footer) {
      const rect = footer.getBoundingClientRect()
      dive = clamp((vh - rect.top) / Math.max(1, Math.min(rect.height, vh * 0.6)), 0, 1)
    }
    return { u, dive, station: f > 0.65 && i < stationU.length - 1 ? i + 1 : i }
  }

  /* State */
  let state = { u: 0, dive: 0, station: -1 }
  let want = { u: 0, dive: 0, station: 0 }
  let stepped = reduced()
  const paused = () => remember('tapiti.walk.paused') === '1'
  if (paused()) stepped = true
  // `?tier=0` (to 3) pins the quality tier, for screenshots on slow software renderers.
  const pinned = params.has('tier')
  let tier = pinned ? clamp(Number(params.get('tier')) || 0, 0, 3) : Number(remember('tapiti.walk.tier', undefined, sessionStorage)) || 0
  if (tier >= 3) flames.visible = false
  let dpr = 1
  const applySize = () => {
    const rect = canvas.getBoundingClientRect()
    const cap = innerWidth < 800 ? 1.25 : 1.5
    dpr = tier >= 1 ? 1 : Math.min(devicePixelRatio || 1, cap)
    renderer.setPixelRatio(dpr)
    renderer.setSize(Math.max(1, Math.round(rect.width)), Math.max(1, Math.round(rect.height)), false)
    camera.aspect = rect.width / Math.max(1, rect.height)
    camera.updateProjectionMatrix()
  }
  let lastHeight = innerHeight
  let resizeTimer = 0
  const onResize = () => {
    clearTimeout(resizeTimer)
    resizeTimer = setTimeout(() => {
      stillDrawn = false
      // The mobile URL bar changes the height by a few percent while scrolling: not a resize.
      const heightOnly = Math.abs(innerHeight - lastHeight) / lastHeight < 0.15 && canvas.width === Math.round(canvas.getBoundingClientRect().width * dpr)
      lastHeight = innerHeight
      measure()
      if (!heightOnly) applySize()
      wake()
    }, 100)
  }

  const minimapSvg = document.querySelector('.minimap-floor svg')
  const you = document.querySelector('.minimap .you')
  const viewBox = minimapSvg ? minimapSvg.viewBox.baseVal : null
  const stops = [...document.querySelectorAll('.minimap .stop')]
  const ticks = [...document.querySelectorAll('.route-list a')]
  let currentStation = -1
  const markStation = (i) => {
    if (i === currentStation) return
    currentStation = i
    for (const [k, el] of stops.entries()) k === i ? el.setAttribute('aria-current', 'true') : el.removeAttribute('aria-current')
    for (const [k, el] of ticks.entries()) k === i ? el.setAttribute('aria-current', 'true') : el.removeAttribute('aria-current')
  }

  let started = performance.now()
  let lastScroll = 0
  let frameId = 0
  let stopped = false
  let first = true
  const samples = []
  let sampleStart = 0
  let prev = performance.now()
  let stillDrawn = false

  const render = (now) => {
    const t = (now - started) / 1000
    const p = orbitMode ? poseOrbit() : leafMode ? poseLeaf(t) : poseAt(state.u, state.dive)
    if (!orbitMode) {
      camera.position.set(p.px, p.py, p.pz)
      camera.lookAt(p.lx, p.ly, p.lz)
      camera.fov = innerWidth < innerHeight * 0.8 ? lerp(50, 60, p.bird) : 50
      // On wide screens the text stands at one side: the picture moves to the other, without turning the camera.
      const w = canvas.width
      const h = canvas.height
      const shift = landingMode && innerWidth >= 1100 ? shiftAt(state.u) * (1 - state.dive) : 0
      if (shift) camera.setViewOffset(w, h, -shift * w, 0, w, h)
      else camera.clearViewOffset()
      camera.updateProjectionMatrix()
    }
    scene.fog.density = leafMode ? 0.0035 : orbitMode ? 0.0003 : lerp(0.0014, 0.0002, p.bird)
    let changed = false
    for (let i = 0; i < count; i++) {
      const level = leafMode ? (i === leafTorch ? 1 : 0) : 1
      if (lit[i] !== level) { lit[i] = level; changed = true }
    }
    if (changed) litAttribute.needsUpdate = true
    renderer.render(scene, camera)
    if (first) {
      first = false
      document.documentElement.classList.add('walk-on')
    }
    // The minimap's "you are here" dot and the current station.
    if (you && viewBox) {
      you.style.left = `${((here[0] - viewBox.x) / viewBox.width) * 100}%`
      you.style.top = `${((here[2] - viewBox.y) / viewBox.height) * 100}%`
    }
    markStation(want.station)
    window.__walk = { tier, calls: renderer.info.render.calls, triangles: renderer.info.render.triangles, stations: built.stations, camera: { x: p.px, y: p.py, z: p.pz, u: state.u }, torches: leafMode ? 1 : count, poolSize: 0, lights: 0, stepped }
  }

  const adapt = (now) => {
    const dt = now - prev
    prev = now
    if (stepped || pinned || dt > 200) return
    if (!sampleStart) sampleStart = now
    samples.push(dt)
    if (now - sampleStart >= 2000) {
      samples.sort((a, b) => a - b)
      const p95 = samples[Math.floor(samples.length * 0.95)]
      samples.length = 0
      sampleStart = 0
      if (p95 > 22 && tier < 3) {
        tier++
        remember('tapiti.walk.tier', String(tier), sessionStorage)
        if (tier === 1) applySize()
        else flames.visible = false
      }
    }
  }

  const step = (now) => {
    frameId = 0
    if (stopped || document.hidden || offscreen) return
    if (leafMode) {
      adapt(now)
      render(now)
      if (stepped) { stillDrawn = true; return }
      frameId = requestAnimationFrame(step)
      return
    }
    want = target()
    if (stepped) want.u = stationU[want.station]
    const k = stepped ? 1 : 1 - Math.exp(-(Math.min(now - prev, 100) / 1000) / 0.18)
    state.u += (want.u - state.u) * k
    state.dive += (want.dive - state.dive) * k
    const moving = Math.abs(want.u - state.u) > 1e-4 || Math.abs(want.dive - state.dive) > 1e-4
    if (!moving) { state.u = want.u; state.dive = want.dive }
    adapt(now)
    if (stepped && stillDrawn && !moving && state.station === want.station) return
    state.station = want.station
    render(now)
    if (stepped) { stillDrawn = true; return }
    if (moving || now - lastScroll < 300) { frameId = requestAnimationFrame(step); return }
    if (state.dive >= 1) return

  }
  let lastTouch = 0
  const stepOrbit = (now) => {
    frameId = 0
    if (stopped || document.hidden) return
    controls.update()
    adapt(now)
    render(now)
    // Damping needs frames only while the orbit camera moves.
    if (now - lastTouch < 900 && !frameId) frameId = requestAnimationFrame(stepOrbit)
  }
  const loop = orbitMode ? stepOrbit : step
  const wake = () => {
    if (stopped || frameId) return
    frameId = requestAnimationFrame(loop)
  }
  if (orbitMode) controls.addEventListener('change', () => { lastTouch = performance.now(); wake() })
  const onScroll = () => { lastScroll = performance.now(); wake() }
  if (landingMode) addEventListener('scroll', onScroll, { passive: true })
  addEventListener('resize', onResize)
  document.addEventListener('visibilitychange', () => { if (!document.hidden) { prev = performance.now(); wake() } })
  canvas.addEventListener('webglcontextlost', (event) => {
    event.preventDefault()
    stopped = true
    document.documentElement.classList.remove('walk-on')
  })
  new ResizeObserver(onResize).observe(canvas)
  // On narrow screens the scene sits behind the entrance only and scrolls away: no frames while it is out of sight.
  let offscreen = false
  new IntersectionObserver(([entry]) => { offscreen = !entry.isIntersecting; if (!offscreen) wake() }).observe(canvas)

  measure()
  applySize()
  want = target()
  state = { u: stepped ? stationU[want.station] : 0, dive: stepped ? want.dive : 0, station: -1 }
  started = performance.now()
  prev = started
  lastScroll = started
  wake()

  const setStepped = (value) => {
    stepped = value
    remember('tapiti.walk.paused', value ? '1' : '0')
    stillDrawn = false
    wake()
  }
  return {
    get stepped() { return stepped },
    pause: () => setStepped(true),
    resume: () => setStepped(reduced()),
    /** `keepContext` keeps the WebGL context, for a rebuild on the same canvas (the lab does that on every regenerate). */
    dispose({ keepContext = false } = {}) {
      stopped = true
      cancelAnimationFrame(frameId)
      removeEventListener('scroll', onScroll)
      removeEventListener('resize', onResize)
      controls?.dispose()
      for (const o of [wallGeometry, plane, texture, wallMaterial, floorMaterial, stepMaterial, brackets.geometry, brackets.material, flames.geometry, flames.material, pillarMesh?.geometry, pillarMesh?.material, ...steps.map((step) => step.geometry)]) o?.dispose()
      renderer.dispose()
      if (!keepContext) renderer.forceContextLoss()
      window.__walk = null
    },
  }
}

/* ---------------- The landing page ---------------- */

async function boot() {
  if (!root || params.get('scene') === 'off' || saveData() || matchMedia('(forced-colors: active)').matches || !hasWebGL2()) return
  const canvas = root.querySelector('canvas')
  const seed = Number(root.dataset.seed)
  const size = Number(root.dataset.size)
  const [wasm, build, THREE, paper] = await Promise.all([import('/lab/pkg/tapiti.js'), import(root.dataset.build), import(root.dataset.three), import(root.dataset.paper)])
  await wasm.default()
  const answer = JSON.parse(wasm.stage_json(seed, wasm.rich_params(size)))
  const floor = answer.floor
  const built = build.build(floor)
  const handle = mount({ THREE, paper, canvas, floor, scene: built, mode: sceneMode, page: document.querySelector('.rooms') })

  const toggle = document.querySelector('.route-pause')
  if (toggle) {
    toggle.hidden = false
    const label = () => {
      const on = handle.stepped
      toggle.setAttribute('aria-pressed', String(on))
      toggle.textContent = on ? toggle.dataset.resume : toggle.dataset.pause
    }
    label()
    toggle.addEventListener('click', () => { handle.stepped ? handle.resume() : handle.pause(); label() })
  }
  addEventListener('pagehide', (event) => { if (!event.persisted) handle.dispose() })
}

if (root) {
  const start = () => boot().catch((error) => console.warn('walk: the scene did not start, the map stays', error))
  const idle = () => (window.requestIdleCallback ? requestIdleCallback(start, { timeout: 2000 }) : setTimeout(start, 200))
  document.readyState === 'complete' ? idle() : addEventListener('load', idle, { once: true })
}
