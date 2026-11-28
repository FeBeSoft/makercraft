//! Formes paramétrées écrites pour MakerCraft (absentes de CadCraft) : étoile et oblong. Elles
//! donnent les sommets d'une polyligne fermée (avec renflements pour les arcs), posée ensuite par
//! la commande `pline` de CadCraft, donc annulable comme le reste.

/// Sommet de polyligne : position (mm) et renflement vers le sommet suivant (0 : droite,
/// 1 : demi-cercle dans le sens trigonométrique).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub x: f64,
    pub y: f64,
    pub bulge: f64,
}

fn place(x: f64, y: f64, center: (f64, f64), rot_deg: f64) -> (f64, f64) {
    let (s, c) = rot_deg.to_radians().sin_cos();
    (center.0 + x * c - y * s, center.1 + x * s + y * c)
}

/// Étoile à `branches` pointes (3 au moins) entre les rayons `outer` (pointes) et `inner`
/// (creux), centrée sur `center`, une pointe vers le haut puis tournée de `rot_deg`.
pub fn star(branches: u32, outer: f64, inner: f64, center: (f64, f64), rot_deg: f64) -> Result<Vec<Vertex>, String> {
    if branches < 3 {
        return Err("une étoile a au moins 3 branches".into());
    }
    if !(outer > 0.0 && inner > 0.0 && inner < outer) {
        return Err("rayons : 0 < intérieur < extérieur".into());
    }
    let n = branches * 2;
    Ok((0..n)
        .map(|k| {
            let r = if k % 2 == 0 { outer } else { inner };
            let a = std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * f64::from(k) / f64::from(n);
            let (x, y) = place(r * a.cos(), r * a.sin(), center, rot_deg);
            Vertex { x, y, bulge: 0.0 }
        })
        .collect())
}

/// Oblong (rainure) de largeur `width`, couché selon X puis tourné de `rot_deg`. `length` est la
/// longueur hors tout si `overall`, sinon l'entraxe des deux demi-cercles.
pub fn oblong(length: f64, width: f64, overall: bool, center: (f64, f64), rot_deg: f64) -> Result<Vec<Vertex>, String> {
    if !(width > 0.0 && length > 0.0) {
        return Err("longueur et largeur positives".into());
    }
    let half_entraxe = if overall { (length - width) / 2.0 } else { length / 2.0 };
    if half_entraxe < 0.0 {
        return Err("longueur hors tout plus petite que la largeur".into());
    }
    let (a, h) = (half_entraxe, width / 2.0);
    // bas de gauche à droite, demi-cercle droit, haut de droite à gauche, demi-cercle gauche
    let pts = [(-a, -h, 0.0), (a, -h, 1.0), (a, h, 0.0), (-a, h, 1.0)];
    Ok(pts
        .iter()
        .map(|&(x, y, bulge)| {
            let (x, y) = place(x, y, center, rot_deg);
            Vertex { x, y, bulge }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_points_alternate_radii() {
        let Ok(v) = star(5, 25.0, 10.0, (0.0, 0.0), 0.0) else { panic!("étoile") };
        assert_eq!(v.len(), 10);
        let r = |p: &Vertex| (p.x * p.x + p.y * p.y).sqrt();
        assert!((r(&v[0]) - 25.0).abs() < 1e-9 && (r(&v[1]) - 10.0).abs() < 1e-9);
        // première pointe vers le haut
        assert!(v[0].x.abs() < 1e-9 && (v[0].y - 25.0).abs() < 1e-9);
        assert!(star(2, 25.0, 10.0, (0.0, 0.0), 0.0).is_err());
        assert!(star(5, 10.0, 25.0, (0.0, 0.0), 0.0).is_err());
    }

    #[test]
    fn oblong_overall_and_entraxe() {
        let Ok(v) = oblong(60.0, 20.0, true, (0.0, 0.0), 0.0) else { panic!("oblong") };
        assert_eq!(v.len(), 4);
        assert!((v[1].x - 20.0).abs() < 1e-9 && (v[1].y + 10.0).abs() < 1e-9 && (v[1].bulge - 1.0).abs() < 1e-12);
        let Ok(e) = oblong(40.0, 20.0, false, (5.0, 5.0), 90.0) else { panic!("oblong") };
        // entraxe 40 couché puis tourné de 90° : centres des arcs sur l'axe Y
        assert!((e[1].x - 15.0).abs() < 1e-9 && (e[1].y - 25.0).abs() < 1e-9);
        assert!(oblong(10.0, 20.0, true, (0.0, 0.0), 0.0).is_err());
    }
}
