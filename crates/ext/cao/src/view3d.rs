//! Vue 3D en lecture seule, dessinée par le moteur de rendu sur processeur de SolidCraft
//! (`solidcraft-viewport`) : brut, brides, contours posés sur le brut, trajectoires.
//! L'application fournit une scène simple ([`Scene3`]) et reçoit une image RGBA.

use std::sync::Arc;

use solidcore_record::Vec3;
use solidcore_tess::Mesh;
use solidcraft_viewport::vec::Bounds;
use solidcraft_viewport::{Camera, Rgb, Scene, SceneLine, SceneMesh};

/// Pavé (brut, bride...).
#[derive(Debug, Clone, PartialEq)]
pub struct Box3 {
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub rgb: [u8; 3],
    /// 1 = opaque.
    pub opacity: f32,
}

/// Polyligne 3D (contour, trajectoire).
#[derive(Debug, Clone, PartialEq)]
pub struct Line3 {
    pub points: Vec<[f64; 3]>,
    pub rgb: [u8; 3],
    pub width: f64,
    /// Dessinée par-dessus les volumes.
    pub on_top: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene3 {
    pub boxes: Vec<Box3>,
    pub lines: Vec<Line3>,
}

/// Point de vue : rotation autour de Z, élévation (radians) et zoom (1 = tout voir).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orbit {
    pub yaw: f64,
    pub pitch: f64,
    pub zoom: f64,
}

impl Default for Orbit {
    fn default() -> Self {
        let c = Camera::default();
        Self { yaw: c.yaw, pitch: c.pitch, zoom: 1.0 }
    }
}

fn v(p: [f64; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

/// Maillage d'un pavé : 4 sommets par face (normales franches), arêtes pour le trait.
fn box_mesh(min: [f64; 3], max: [f64; 3]) -> Mesh {
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    // chaque face : 4 coins dans le sens trigonométrique vu de l'extérieur, et sa normale
    let faces: [([[f64; 3]; 4], [f64; 3]); 6] = [
        ([[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]], [0.0, 0.0, 1.0]),
        ([[x0, y1, z0], [x1, y1, z0], [x1, y0, z0], [x0, y0, z0]], [0.0, 0.0, -1.0]),
        ([[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]], [0.0, -1.0, 0.0]),
        ([[x1, y1, z0], [x0, y1, z0], [x0, y1, z1], [x1, y1, z1]], [0.0, 1.0, 0.0]),
        ([[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]], [-1.0, 0.0, 0.0]),
        ([[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]], [1.0, 0.0, 0.0]),
    ];
    let mut m = Mesh::default();
    for (fi, (corners, n)) in faces.iter().enumerate() {
        let base = m.positions.len() as u32;
        for c in corners {
            m.positions.push(v(*c));
            m.normals.push(v(*n));
        }
        m.triangles.push([base, base + 1, base + 2]);
        m.triangles.push([base, base + 2, base + 3]);
        m.tri_face.push(fi as u32);
        m.tri_face.push(fi as u32);
    }
    let c = |x, y, z| v([x, y, z]);
    m.edges = vec![
        vec![c(x0, y0, z0), c(x1, y0, z0), c(x1, y1, z0), c(x0, y1, z0), c(x0, y0, z0)],
        vec![c(x0, y0, z1), c(x1, y0, z1), c(x1, y1, z1), c(x0, y1, z1), c(x0, y0, z1)],
        vec![c(x0, y0, z0), c(x0, y0, z1)],
        vec![c(x1, y0, z0), c(x1, y0, z1)],
        vec![c(x1, y1, z0), c(x1, y1, z1)],
        vec![c(x0, y1, z0), c(x0, y1, z1)],
    ];
    m.display_only_edges = true;
    m
}

fn rgb(c: [u8; 3]) -> Rgb {
    Rgb(c[0], c[1], c[2])
}

impl Scene3 {
    pub fn bounds(&self) -> Bounds {
        let mut b = Bounds::default();
        for bx in &self.boxes {
            b.add(v(bx.min));
            b.add(v(bx.max));
        }
        for l in &self.lines {
            for p in &l.points {
                b.add(v(*p));
            }
        }
        b
    }

    fn to_scene(&self, background: ([u8; 3], [u8; 3])) -> Scene {
        let mut s = Scene { background: Some((rgb(background.0), rgb(background.1))), ..Scene::default() };
        for bx in &self.boxes {
            let mut m = SceneMesh::solid(Arc::new(box_mesh(bx.min, bx.max)), rgb(bx.rgb));
            m.opacity = bx.opacity;
            s.meshes.push(m);
        }
        for l in &self.lines {
            if l.points.len() < 2 {
                continue;
            }
            s.lines.push(SceneLine { points: l.points.iter().map(|p| v(*p)).collect(), color: rgb(l.rgb), alpha: 1.0, width: l.width, on_top: l.on_top });
        }
        s.radius = self.bounds().diagonal() / 2.0;
        s
    }
}

/// Dessine la scène en `w` x `h` pixels ; renvoie l'image RGBA (vide si la scène l'est).
pub fn render(scene: &Scene3, orbit: &Orbit, w: usize, h: usize, background: ([u8; 3], [u8; 3])) -> Vec<u8> {
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let b = scene.bounds();
    let mut cam = Camera { yaw: orbit.yaw, pitch: orbit.pitch, ..Camera::default() };
    if !b.is_empty() {
        cam.fit_aspect(&b, w as f64 / h as f64);
        cam.distance /= orbit.zoom.clamp(0.05, 50.0);
    }
    let canvas = solidcraft_viewport::render(&scene.to_scene(background), &cam, w, h);
    if canvas.w == w && canvas.h == h { canvas.rgba } else { Vec::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_mesh_is_closed_and_consistent() {
        let m = box_mesh([0.0, 0.0, 0.0], [10.0, 5.0, 2.0]);
        assert_eq!(m.positions.len(), 24);
        assert_eq!(m.triangles.len(), 12);
        assert_eq!(m.tri_face.len(), 12);
        assert_eq!(m.normals.len(), m.positions.len());
    }

    #[test]
    fn renders_a_stock_with_its_outline() {
        let scene = Scene3 {
            boxes: vec![Box3 { min: [0.0, 0.0, -10.0], max: [100.0, 60.0, 0.0], rgb: [190, 150, 100], opacity: 1.0 }],
            lines: vec![Line3 { points: vec![[10.0, 10.0, 0.1], [90.0, 10.0, 0.1], [90.0, 50.0, 0.1]], rgb: [80, 140, 255], width: 2.0, on_top: true }],
        };
        let img = render(&scene, &Orbit::default(), 160, 120, ([20, 22, 28], [30, 33, 40]));
        assert_eq!(img.len(), 160 * 120 * 4);
        // le brut occupe le centre : le pixel central n'est pas le fond
        let i = (60 * 160 + 80) * 4;
        assert_ne!(&img[i..i + 3], &[20, 22, 28]);
    }
}
