// Shared drawing materials for the first-person walk, the lab orbit view and the leaf chamber.
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
        float grain(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
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
        diffuseColor.rgb *= 0.95 + 0.05 * grain(floor(vPencilUv * 4.0));
        diffuseColor.rgb = mix(diffuseColor.rgb, uInk, max(joint * 0.54, stroke * shade));`)
  }
  material.customProgramCacheKey = () => 'tapiti-pencil'
  return material
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
