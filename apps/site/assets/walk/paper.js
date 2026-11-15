// Shared drawing materials for the first-person walk, the lab plan/orbit view and the leaf chamber.
// Everything in the walk is made by code: no image is loaded. Surfaces are drawn per pixel in world units (so they stay sharp at any
// distance and screen size), props are boxes, and every random choice comes from a hash.

/** Hash, value noise and a crack line, shared by the surface shaders. */
const NOISE = `
  float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
  float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), f.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), f.x), f.y);
  }
  float fbm(vec2 p) { return 0.5 * vnoise(p) + 0.25 * vnoise(p * 2.03 + 7.1) + 0.125 * vnoise(p * 4.1 + 3.7); }
  // A jagged crack from the top edge of a cell (fr in 0..1, aa = fwidth of the cell coordinates), 1 on the line.
  float crackLine(vec2 fr, vec2 aa, float h) {
    float x0 = 0.25 + 0.5 * fract(h * 7.3);
    float jag = (vnoise(vec2(fr.y * 6.0, h * 40.0)) - 0.5) * 0.16 + (vnoise(vec2(fr.y * 24.0, h * 9.0)) - 0.5) * 0.04;
    float d = abs(fr.x - x0 - jag) / max(aa.x, 1e-5);
    float reach = 0.25 + 0.4 * fract(h * 3.1);
    float along = smoothstep(1.0 - reach, 1.0 - reach + 0.12, fr.y);
    return (1.0 - smoothstep(0.5, 1.4, d)) * along;
  }
`
export function hatchPatterns(context, ink) {
  const patterns = {}
  for (const cls of ['disc', 'oct', 'L', 'star']) {
    const tile = document.createElement('canvas')
    tile.width = tile.height = 10
    const pen = tile.getContext('2d')
    pen.fillStyle = pen.strokeStyle = ink
    pen.lineWidth = 1
    pen.beginPath()
    if (cls === 'disc') {
      pen.arc(5, 5, 1.2, 0, Math.PI * 2)
      pen.fill()
    } else {
      if (cls === 'oct') { pen.moveTo(0, 10); pen.lineTo(10, 0) }
      else { pen.moveTo(0, 5); pen.lineTo(10, 5) }
      if (cls === 'star') { pen.moveTo(5, 0); pen.lineTo(5, 10) }
      pen.stroke()
    }
    patterns[cls] = context.createPattern(tile, 'repeat')
  }
  return patterns
}

export function wallMaterial(THREE, color, ink) {
  const material = new THREE.MeshBasicMaterial({ color })
  material.onBeforeCompile = (shader) => {
    shader.uniforms.uInk = { value: new THREE.Color(ink) }
    shader.vertexShader = shader.vertexShader
      .replace('void main() {', 'varying vec2 vPencilUv;\nvoid main() {')
      .replace('#include <begin_vertex>', '#include <begin_vertex>\nvPencilUv = uv;')
    shader.fragmentShader = shader.fragmentShader
      .replace('void main() {', `varying vec2 vPencilUv;
        uniform vec3 uInk;
        ${NOISE}
        void main() {`)
      .replace('#include <color_fragment>', `#include <color_fragment>
        vec2 pen = vPencilUv + vec2(sin(vPencilUv.y * 0.39) * 0.28, sin(vPencilUv.x * 0.27) * 0.22);
        vec2 g = pen / vec2(30.0, 15.0);
        g.x += floor(g.y) * 0.48;
        vec2 f = min(fract(g), 1.0 - fract(g)) / max(fwidth(g), vec2(1e-5));
        float joint = 1.0 - smoothstep(0.35, 1.25, min(f.x, f.y));
        float q = (pen.x + pen.y * 0.65) / 2.4;
        float stroke = 1.0 - smoothstep(0.25, 1.0, abs(fract(q) - 0.5) / max(fwidth(q), 1e-5));
        float shade = 0.18 + 0.24 * (1.0 - smoothstep(0.0, 28.0, vPencilUv.y));
        vec2 brick = floor(g);
        float bh = hash(brick + 11.0);
        float crack = crackLine(fract(g), fwidth(g), bh) * step(0.82, bh);
        diffuseColor.rgb *= 0.93 + 0.1 * bh + 0.05 * (fbm(vPencilUv * 0.11) - 0.5);
        diffuseColor.rgb = mix(diffuseColor.rgb, uInk, max(max(joint * 0.54, stroke * shade), crack * 0.8));`)
  }
  material.customProgramCacheKey = () => 'tapiti-pencil-2'
  return material
}

/** Leaves the stairwell open: discards the pixels of `material` that lie inside `well` ([x0, z0, x1, z1]). */
function openWell(material, well, install) {
  material.onBeforeCompile = (shader) => {
    shader.uniforms.uWell = { value: Float32Array.from(well) }
    shader.vertexShader = shader.vertexShader.replace('void main() {', 'varying vec3 vWorld;\nvoid main() {').replace('#include <begin_vertex>', '#include <begin_vertex>\nvWorld = (modelMatrix * vec4(position, 1.0)).xyz;')
    shader.fragmentShader = shader.fragmentShader.replace('void main() {', 'uniform vec4 uWell;\nvarying vec3 vWorld;\n' + (install.header || '') + '\nvoid main() {\n  if (vWorld.x > uWell.x && vWorld.x < uWell.z && vWorld.z > uWell.y && vWorld.z < uWell.w) discard;')
    install.body?.(shader)
  }
}

/** The floor (flagstones) or the ceiling (larger slabs, damp stains), drawn per pixel from world coordinates. `map` carries the outline in its alpha. */
export function slabMaterial(THREE, { color, ink, map, well, ceiling, aoStep }) {
  const material = new THREE.MeshBasicMaterial({ color, map, alphaTest: 0.1, side: ceiling ? THREE.DoubleSide : THREE.FrontSide })
  openWell(material, well, {
    header: `uniform vec3 uInk;\nuniform vec2 uAo;\n${NOISE}`,
    body(shader) {
      shader.uniforms.uInk = { value: new THREE.Color(ink) }
      shader.uniforms.uAo = { value: new THREE.Vector2(aoStep[0], aoStep[1]) }
      const size = ceiling ? 'vec2(44.0, 26.0)' : 'vec2(28.0, 28.0)'
      shader.fragmentShader = shader.fragmentShader.replace('#include <color_fragment>', `#include <color_fragment>
        float mask = diffuseColor.a;
        diffuseColor.rgb = diffuse;
        vec2 wp = vWorld.xz;
        vec2 g = wp / ${size};
        float row = floor(g.y);
        g.x += row * 0.5 + hash(vec2(row, 3.0)) * 0.3;
        vec2 id = floor(g);
        vec2 fr = fract(g);
        vec2 aa = fwidth(g);
        float h = hash(id + ${ceiling ? '5.0' : '1.0'});
        vec2 f = min(fr, 1.0 - fr) / max(aa, vec2(1e-5));
        float joint = 1.0 - smoothstep(0.4, 1.5, min(f.x, f.y));
        float q = (wp.x + wp.y) / ${ceiling ? '2.2' : '2.8'};
        float stroke = 1.0 - smoothstep(0.25, 1.0, abs(fract(q) - 0.5) / max(fwidth(q), 1e-5));
        float worn = fbm(wp * 0.045);
        float hatch = stroke * step(h, ${ceiling ? '0.55' : '0.35'}) * (0.18 + 0.4 * worn);
        float crack = crackLine(fr, aa, h) * step(${ceiling ? '0.78' : '0.84'}, h);
        diffuseColor.rgb *= 0.92 + 0.1 * h + 0.06 * (worn - 0.5);
        ${ceiling ? 'diffuseColor.rgb *= 1.0 - 0.22 * smoothstep(0.55, 0.78, fbm(wp * 0.02 + 9.0));' : ''}
        // Darker where the stone meets the wall: the outline, sampled a little to each side, tells how close the wall is.
        float open = (texture2D(map, vMapUv + vec2(uAo.x, 0.0)).a + texture2D(map, vMapUv - vec2(uAo.x, 0.0)).a + texture2D(map, vMapUv + vec2(0.0, uAo.y)).a + texture2D(map, vMapUv - vec2(0.0, uAo.y)).a) * 0.25;
        diffuseColor.rgb *= mix(0.8, 1.0, open);
        diffuseColor.rgb = mix(diffuseColor.rgb, uInk, max(max(joint * 0.5, hatch), crack * 0.8));
        diffuseColor.a = mask;`)
    },
  })
  material.customProgramCacheKey = () => (ceiling ? 'tapiti-ceiling' : 'tapiti-floor')
  return material
}

/** The pencilled route: a dashed ribbon with a wobbling edge, dashes measured along the walk in world units. */
export function routeMaterial(THREE, { color, well }) {
  const material = new THREE.MeshBasicMaterial({ color, transparent: true, depthWrite: false, polygonOffset: true, polygonOffsetFactor: -2, polygonOffsetUnits: -2 })
  openWell(material, well, {
    header: `varying vec2 vRoute;\n${NOISE}`,
    body(shader) {
      shader.vertexShader = shader.vertexShader.replace('void main() {', 'varying vec2 vRoute;\nvoid main() {').replace('#include <begin_vertex>', '#include <begin_vertex>\nvRoute = uv;')
      shader.fragmentShader = shader.fragmentShader.replace('#include <color_fragment>', `#include <color_fragment>
        float along = vRoute.x;
        float t = abs(fract(along / 16.0) - 0.5) * 16.0;
        float soft = max(fwidth(along) * 0.7, 0.04);
        float dash = 1.0 - smoothstep(4.6 - soft, 4.6 + soft, t + (vnoise(vec2(along * 0.2, 2.0)) - 0.5) * 0.5);
        float across = abs(vRoute.y) + (vnoise(vec2(along * 0.12, 9.0)) - 0.5) * 0.1;
        float edge = 1.0 - smoothstep(0.72 - max(fwidth(vRoute.y), 0.02), 0.95, across);
        diffuseColor.a *= dash * edge * (0.8 + 0.2 * vnoise(vec2(along * 0.2, vRoute.y * 3.0)));`)
    },
  })
  material.customProgramCacheKey = () => 'tapiti-route'
  return material
}

/** Cobwebs: spokes and sagging rings drawn over a triangle (apex at the corner), as thin ink lines. */
export function webMaterial(THREE, ink) {
  const material = new THREE.MeshBasicMaterial({ color: ink, transparent: true, depthWrite: false, side: THREE.DoubleSide })
  material.onBeforeCompile = (shader) => {
    shader.vertexShader = shader.vertexShader.replace('void main() {', 'varying vec2 vWeb;\nvoid main() {').replace('#include <begin_vertex>', '#include <begin_vertex>\nvWeb = uv;')
    shader.fragmentShader = shader.fragmentShader.replace('void main() {', `varying vec2 vWeb;
      void main() {`).replace('#include <color_fragment>', `#include <color_fragment>
        float s = vWeb.x + vWeb.y;
        float t = vWeb.y / max(s, 1e-4);
        float k = t * 5.0;
        float spoke = 1.0 - smoothstep(0.4, 1.3, abs(k - floor(k + 0.5)) / max(fwidth(k), 1e-5));
        float x = fract(k);
        float c = s * 8.0 - (1.0 - 4.0 * (x - 0.5) * (x - 0.5)) * 0.35;
        float ring = 1.0 - smoothstep(0.4, 1.3, min(fract(c), 1.0 - fract(c)) / max(fwidth(c), 1e-5));
        ring *= step(1.0, c) * 0.8;
        diffuseColor.a *= max(spoke, ring) * 0.7 * (1.0 - smoothstep(0.88, 1.0, s));`)
  }
  material.customProgramCacheKey = () => 'tapiti-web'
  return material
}

/** A triangle per web, uv = barycentric from the apex: positions [x, z, dx1, dz1, dx2, dz2, size] in, geometry out. */
export function webGeometry(THREE, webs, height) {
  const n = webs.length / 7
  const positions = new Float32Array(n * 9)
  const uvs = new Float32Array(n * 6)
  for (let i = 0; i < n; i++) {
    const [x, z, ax, az, bx, bz, size] = webs.subarray(i * 7, i * 7 + 7)
    positions.set([x, height, z, x + ax * size, height, z + az * size, x + bx * size, height, z + bz * size], i * 9)
    uvs.set([0, 0, 1, 0, 0, 1], i * 6)
  }
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3))
  geometry.setAttribute('uv', new THREE.BufferAttribute(uvs, 2))
  return geometry
}

/** A solid of revolution about the x axis: `profile` is [x, radius] pairs, `sides` the segments round it. Vertex colours shade the underside darker. */
function lathe(THREE, profile, sides, shade = 1) {
  const positions = []
  const colors = []
  const indices = []
  profile.forEach(([x, r]) => {
    for (let j = 0; j < sides; j++) {
      const t = (j / sides) * Math.PI * 2
      positions.push(x, r * Math.cos(t), r * Math.sin(t))
      colors.push(...Array(3).fill(shade * (0.62 + 0.38 * Math.max(0, Math.cos(t)))))
    }
  })
  for (let i = 0; i < profile.length - 1; i++) {
    for (let j = 0; j < sides; j++) {
      const a = i * sides + j
      const b = i * sides + ((j + 1) % sides)
      indices.push(a, b, a + sides, b, b + sides, a + sides)
    }
  }
  const g = new THREE.BufferGeometry()
  g.setAttribute('position', new THREE.BufferAttribute(new Float32Array(positions), 3))
  g.setAttribute('color', new THREE.BufferAttribute(new Float32Array(colors), 3))
  g.setIndex(indices)
  return g
}

/** A bone (a thin shaft with a rounded knob at each end) and a skull (a rounded cranium, a jaw and two dark sockets), lying on the floor. */
export function propGeometries(THREE) {
  const merge = (parts) => THREE.mergeGeometries(parts)
  const knob = (x, z) => lathe(THREE, [[-1.5, 0.01], [-1, 1.2], [0, 1.7], [1, 1.2], [1.5, 0.01]], 8).translate(x, 1.7, z)
  const ball = (r, sx, sy, sz, x, y, z, shade) => lathe(THREE, [[-r, 0.01], [-r * 0.7, r * 0.72], [0, r], [r * 0.7, r * 0.72], [r, 0.01]], 10, shade).scale(sx, sy, sz).translate(x, y, z)
  return {
    bone: merge([lathe(THREE, [[-5, 0.4], [-4, 1], [0, 0.85], [4, 1], [5, 0.4]], 6).translate(0, 1.1, 0), knob(-5.3, -0.9), knob(-5.3, 1), knob(5.3, -0.9), knob(5.3, 1)]),
    skull: merge([ball(3.3, 1, 0.9, 1, 0, 3, 0, 1), ball(2.2, 1, 0.55, 0.9, 0.2, 1.3, 2.6, 0.92), ball(0.7, 1, 1, 0.5, -1.1, 3.2, 3.1, 0.12), ball(0.7, 1, 1, 0.5, 1.1, 3.2, 3.1, 0.12)]),
  }
}

export const FLAME_VERTEX = `
  attribute float lit;
  varying vec2 vUv;
  varying float vLevel;
  void main() {
    vUv = uv;
    vLevel = lit;
    vec4 mv = modelViewMatrix * instanceMatrix * vec4(0.0, 0.0, 0.0, 1.0);
    mv.xy += position.xy * vec2(12.0, 20.0);
    mv.y += 6.0;
    gl_Position = projectionMatrix * mv;
  }
`

export const FLAME_FRAGMENT = `
  uniform vec3 uColor;
  uniform vec3 uCore;
  uniform vec3 uInk;
  varying vec2 vUv;
  varying float vLevel;
  void main() {
    float y = clamp(vUv.y, 0.001, 0.999);
    float width = pow(sin(y * 3.14159), 0.75) * (0.39 - y * 0.25);
    float x = vUv.x - 0.5 - sin(y * 7.0) * 0.045;
    float d = abs(x) - width + sin(y * 61.0) * 0.006;
    float aa = max(fwidth(d), 0.003);
    float ink = 1.0 - smoothstep(aa * 0.7, aa * 1.8, abs(d));
    float pencil = 1.0 - smoothstep(0.02, 0.1, abs(fract((vUv.x + y * 0.5) * 14.0) - 0.5));
    vec3 wash = mix(uColor, uCore, 1.0 - smoothstep(0.0, 0.3, length(vec2(x, y - 0.3))));
    gl_FragColor = vec4(mix(wash, uInk, max(ink * 0.85, pencil * 0.16)), (1.0 - smoothstep(0.0, aa, d)) * vLevel);
    #include <colorspace_fragment>
  }
`
