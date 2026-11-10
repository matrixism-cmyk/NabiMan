/** The scene round the graph: its lights, a far field of faint stars, and the camera's flight to a node. */
import * as THREE from 'three';
import type { Three } from './nodeParts';

export function addLights(scene: Three) {
  scene.add(new THREE.AmbientLight('#8fb4ff', 1.1));
  const key = new THREE.PointLight('#ffffff', 900, 0, 1.6);
  key.position.set(120, 160, 220);
  scene.add(key);
}

/** Faint stars far behind, for depth as the view turns; returns their material, whose opacity the style sets. */
export function addStars(scene: Three) {
  const stars = new THREE.BufferGeometry();
  const pts = new Float32Array(900 * 3);
  for (let i = 0; i < pts.length; i += 1) pts[i] = (Math.random() - 0.5) * 2400;
  stars.setAttribute('position', new THREE.BufferAttribute(pts, 3));
  const mat = new THREE.PointsMaterial({ color: '#7c93b8', size: 1.6, transparent: true, opacity: 0.55, sizeAttenuation: true });
  const points = new THREE.Points(stars, mat);
  scene.add(points);
  return points;
}

interface At { x?: number; y?: number; z?: number }

/** Moves the camera to look at a node from a little way out, along the line from the middle. */
export function flyTo(g: Three, n: At, ms = 900) {
  const d = Math.hypot(n.x || 0, n.y || 0, n.z || 0) || 1;
  const k = 1 + 120 / d;
  g.cameraPosition({ x: (n.x || 0) * k, y: (n.y || 0) * k, z: (n.z || 0) * k + (d < 5 ? 150 : 0) }, n, ms);
}
