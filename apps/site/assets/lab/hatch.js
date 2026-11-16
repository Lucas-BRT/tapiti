// Hatch tiles for the plan: one pattern per room shape, so shape is not carried by colour alone.
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
