use rand::prelude::*;
use rayon::prelude::*;
use std::sync::Arc;

#[derive(Copy, Clone, Default, Debug)]
struct Vec3(f32, f32, f32);

impl Vec3 {
    pub fn dot(&self, other: &Self) -> f32 {
        self.0 * other.0 + self.1 * other.1 + self.2 * other.2
    }

    pub fn cross(&self, other: &Self) -> Self {
        Vec3(
            self.1 * other.2 - self.2 * other.1,
            -(self.0 * other.2 - self.2 * other.0),
            self.0 * other.1 - self.1 * other.0,
        )
    }

    pub fn length(&self) -> f32 {
        f32::sqrt(self.dot(self))
    }

    pub fn into_unit(self) -> Self {
        self / self.length()
    }
}

fn reflect(v: &Vec3, n: &Vec3) -> Vec3 {
    *v - 2. * v.dot(n) * *n
}

fn refract(v: &Vec3, n: &Vec3, ni_over_nt: f32) -> Option<Vec3> {
    let uv = v.into_unit();
    let dt = uv.dot(n);
    let discriminant = 1.0 - ni_over_nt * ni_over_nt * (1. - dt * dt);
    if discriminant > 0. {
        Some(ni_over_nt * (uv - dt * *n) - discriminant.sqrt() * *n)
    } else {
        None
    }
}

impl std::ops::Mul<Vec3> for f32 {
    type Output = Vec3;

    fn mul(self, rhs: Vec3) -> Self::Output {
        Vec3(self * rhs.0, self * rhs.1, self * rhs.2)
    }
}

impl std::ops::Mul for Vec3 {
    type Output = Vec3;

    fn mul(self, rhs: Vec3) -> Self::Output {
        Vec3(self.0 * rhs.0, self.1 * rhs.1, self.2 * rhs.2)
    }
}

impl std::ops::Div<f32> for Vec3 {
    type Output = Vec3;

    fn div(self, rhs: f32) -> Self::Output {
        Vec3(self.0 / rhs, self.1 / rhs, self.2 / rhs)
    }
}

impl std::ops::Add for Vec3 {
    type Output = Vec3;

    fn add(self, rhs: Vec3) -> Self::Output {
        Vec3(self.0 + rhs.0, self.1 + rhs.1, self.2 + rhs.2)
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Vec3;

    fn sub(self, rhs: Vec3) -> Self::Output {
        Vec3(self.0 - rhs.0, self.1 - rhs.1, self.2 - rhs.2)
    }
}

impl std::ops::Neg for Vec3 {
    type Output = Vec3;

    fn neg(self) -> Self::Output {
        Vec3(-self.0, -self.1, -self.2)
    }
}

impl std::iter::Sum for Vec3 {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.fold(Vec3::default(), |sum, x| sum + x)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Channel {
    R,
    G,
    B,
}

use Channel::*;

impl ::std::ops::Index<Channel> for Vec3 {
    type Output = f32;

    fn index(&self, idx: Channel) -> &Self::Output {
        match idx {
            R => &self.0,
            G => &self.1,
            B => &self.2,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Axis {
    X,
    Y,
    Z,
}

use Axis::*;

impl ::std::ops::Index<Axis> for Vec3 {
    type Output = f32;

    fn index(&self, idx: Axis) -> &Self::Output {
        match idx {
            X => &self.0,
            Y => &self.1,
            Z => &self.2,
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct Ray {
    origin: Vec3,
    direction: Vec3,
}

impl Ray {
    pub fn point_at_parameter(&self, t: f32) -> Vec3 {
        self.origin + t * self.direction
    }
}

#[derive(Clone, Debug)]
pub struct HitRecord {
    t: f32,
    p: Vec3,
    normal: Vec3,
    material: Arc<dyn Material>,
}

trait Object: Sync + Send {
    fn hit(&self, ray: &Ray, t_range: std::ops::Range<f32>) -> Option<HitRecord>;
}

#[derive(Clone, Debug)]
pub struct Sphere {
    center: Vec3,
    radius: f32,
    material: Arc<dyn Material>,
}

impl Object for Sphere {
    fn hit(&self, ray: &Ray, t_range: std::ops::Range<f32>) -> Option<HitRecord> {
        let oc = ray.origin - self.center;
        let a = ray.direction.dot(&ray.direction);
        let b = oc.dot(&ray.direction);
        let c = oc.dot(&oc) - self.radius * self.radius;
        let discriminant = b * b - a * c;
        if discriminant > 0. {
            for &t in &[
                (-b - discriminant.sqrt()) / a,
                (-b + discriminant.sqrt()) / a,
            ] {
                if t < t_range.end && t >= t_range.start {
                    let p = ray.point_at_parameter(t);
                    return Some(HitRecord {
                        t,
                        p,
                        normal: (p - self.center) / self.radius,
                        material: Arc::clone(&self.material),
                    });
                }
            }
        }
        None
    }
}

impl<T> Object for [T]
where
    T: std::ops::Deref<Target = dyn Object> + Send + Sync,
{
    fn hit(&self, ray: &Ray, t_range: std::ops::Range<f32>) -> Option<HitRecord> {
        self.iter().fold(None, |hit, obj| {
            if let Some(rec) = obj.hit(ray, t_range.clone()) {
                let hit_t = hit.as_ref().map(|h| h.t).unwrap_or(t_range.end);
                if rec.t < hit_t {
                    return Some(rec);
                }
            }
            hit
        })
    }
}

fn color(world: &[Box<dyn Object>], mut ray: Ray) -> Vec3 {
    let mut strength = Vec3(1., 1., 1.);
    let mut bounces = 0;

    while let Some(hit) = world.hit(&ray, 0.001..std::f32::MAX) {
        if bounces < 50 {
            if let Some((new_ray, attenuation)) = hit.material.scatter(&ray, &hit) {
                ray = new_ray;
                strength = Vec3(
                    strength.0 * attenuation.0,
                    strength.1 * attenuation.1,
                    strength.2 * attenuation.2,
                );
                bounces += 1;
                continue;
            }
        }
        return Vec3::default();
    }

    let unit_direction = ray.direction.into_unit();
    let t = 0.5 * (unit_direction[Y] + 1.0);
    let col = (1. - t) * Vec3(1., 1., 1.) + t * Vec3(0.5, 0.7, 1.0);
    Vec3(strength.0 * col.0, strength.1 * col.1, strength.2 * col.2)
}

struct Camera {
    origin: Vec3,
    lower_left_corner: Vec3,
    horizontal: Vec3,
    vertical: Vec3,
    u: Vec3,
    v: Vec3,
    lens_radius: f32,
}

impl Camera {
    fn look(
        look_from: Vec3,
        look_at: Vec3,
        up: Vec3,
        fov: f32,
        aspect: f32,
        aperture: f32,
        focus_dist: f32,
    ) -> Self {
        let lens_radius = aperture / 2.;
        let theta = fov * std::f32::consts::PI / 180.;
        let half_height = f32::tan(theta / 2.);
        let half_width = aspect * half_height;
        let origin = look_from;
        let w = (look_from - look_at).into_unit();
        let u = up.cross(&w).into_unit();
        let v = w.cross(&u);
        let lower_left_corner =
            origin - half_width * focus_dist * u - half_height * focus_dist * v - focus_dist * w;
        let horizontal = 2. * half_width * focus_dist * u;
        let vertical = 2. * half_height * focus_dist * v;
        Camera {
            origin,
            lower_left_corner,
            horizontal,
            vertical,
            u,
            v,
            lens_radius,
        }
    }

    fn get_ray(&self, s: f32, t: f32) -> Ray {
        let rd = self.lens_radius * in_unit_disc();
        let offset = rd[X] * self.u + rd[Y] * self.v;
        Ray {
            origin: self.origin + offset,
            direction: self.lower_left_corner + s * self.horizontal + t * self.vertical
                - self.origin
                - offset,
        }
    }
}

fn in_unit_sphere() -> Vec3 {
    let mut rng = rand::thread_rng();
    loop {
        let v = 2. * Vec3(rng.gen(), rng.gen(), rng.gen()) - Vec3(1., 1., 1.);
        if v.dot(&v) < 1. {
            return v;
        }
    }
}

fn in_unit_disc() -> Vec3 {
    let mut rng = rand::thread_rng();
    loop {
        let v = 2. * Vec3(rng.gen(), rng.gen(), 0.) - Vec3(1., 1., 0.);
        if v.dot(&v) < 1. {
            return v;
        }
    }
}

trait Material: std::fmt::Debug + Sync + Send {
    fn scatter(&self, ray: &Ray, hit: &HitRecord) -> Option<(Ray, Vec3)>;
}

#[derive(Debug)]
struct Lambertian {
    albedo: Vec3,
}

impl Material for Lambertian {
    fn scatter(&self, _ray: &Ray, hit: &HitRecord) -> Option<(Ray, Vec3)> {
        let target = hit.p + hit.normal + in_unit_sphere();
        let scattered = Ray {
            origin: hit.p,
            direction: target - hit.p,
        };
        Some((scattered, self.albedo))
    }
}

#[derive(Debug)]
struct Metal {
    albedo: Vec3,
    fuzz: f32,
}

impl Material for Metal {
    fn scatter(&self, ray: &Ray, hit: &HitRecord) -> Option<(Ray, Vec3)> {
        let scattered = Ray {
            origin: hit.p,
            direction: reflect(&ray.direction.into_unit(), &hit.normal)
                + self.fuzz * in_unit_sphere(),
        };
        if scattered.direction.dot(&hit.normal) > 0. {
            Some((scattered, self.albedo))
        } else {
            None
        }
    }
}

fn schlick(cos: f32, ref_idx: f32) -> f32 {
    let r0 = (1. - ref_idx) / (1. + ref_idx);
    let r0 = r0 * r0;
    r0 + (1. - r0) * f32::powf(1. - cos, 5.)
}

#[derive(Debug)]
struct Dielectric {
    ref_idx: f32,
}

impl Material for Dielectric {
    fn scatter(&self, ray: &Ray, hit: &HitRecord) -> Option<(Ray, Vec3)> {
        let (outward_normal, ni_over_nt, cosine) = if ray.direction.dot(&hit.normal) > 0. {
            (
                -hit.normal,
                self.ref_idx,
                self.ref_idx * ray.direction.dot(&hit.normal) / ray.direction.length(),
            )
        } else {
            (
                hit.normal,
                1.0 / self.ref_idx,
                -ray.direction.dot(&hit.normal) / ray.direction.length(),
            )
        };

        let direction = refract(&ray.direction, &outward_normal, ni_over_nt)
            .filter(|_| rand::thread_rng().gen::<f32>() >= schlick(cosine, self.ref_idx))
            .unwrap_or_else(|| reflect(&ray.direction, &hit.normal));

        let attenuation = Vec3(1.0, 1.0, 1.0);
        let ray = Ray {
            origin: hit.p,
            direction,
        };
        Some((ray, attenuation))
    }
}

fn random_scene() -> Vec<Box<dyn Object>> {
    let mut world: Vec<Box<dyn Object>> = vec![Box::new(Sphere {
        center: Vec3(0., -1000., 0.),
        radius: 1000.,
        material: Arc::new(Lambertian {
            albedo: Vec3(0.5, 0.5, 0.5),
        }),
    })];

    let mut rng = rand::thread_rng();

    for a in -11..11 {
        for b in -11..11 {
            let center = Vec3(
                a as f32 + 0.9 * rng.gen::<f32>(),
                0.2,
                b as f32 + 0.9 * rng.gen::<f32>(),
            );
            if (center - Vec3(4., 0.2, 0.)).length() > 0.9 {
                let choose_mat = rng.gen::<f32>();

                let obj = if choose_mat < 0.8 {
                    Box::new(Sphere {
                        center,
                        radius: 0.2,
                        material: Arc::new(Lambertian {
                            albedo: Vec3(
                                rng.gen::<f32>() * rng.gen::<f32>(),
                                rng.gen::<f32>() * rng.gen::<f32>(),
                                rng.gen::<f32>() * rng.gen::<f32>(),
                            ),
                        }),
                    })
                } else if choose_mat < 0.95 {
                    Box::new(Sphere {
                        center,
                        radius: 0.2,
                        material: Arc::new(Metal {
                            albedo: Vec3(
                                0.5 * (1. + rng.gen::<f32>()),
                                0.5 * (1. + rng.gen::<f32>()),
                                0.5 * (1. + rng.gen::<f32>()),
                            ),
                            fuzz: 0.5 * rng.gen::<f32>(),
                        }),
                    })
                } else {
                    Box::new(Sphere {
                        center,
                        radius: 0.2,
                        material: Arc::new(Dielectric { ref_idx: 1.5 }),
                    })
                };
                world.push(obj);
            }
        }
    }

    world.push(Box::new(Sphere {
        center: Vec3(0., 1., 0.),
        radius: 1.0,
        material: Arc::new(Dielectric { ref_idx: 1.5 }),
    }));

    world.push(Box::new(Sphere {
        center: Vec3(-4., 1., 0.),
        radius: 1.0,
        material: Arc::new(Lambertian {
            albedo: Vec3(0.4, 0.2, 0.1),
        }),
    }));

    world.push(Box::new(Sphere {
        center: Vec3(4., 1., 0.),
        radius: 1.0,
        material: Arc::new(Metal {
            albedo: Vec3(0.7, 0.6, 0.5),
            fuzz: 0.,
        }),
    }));

    world
}

struct Image(Vec<Vec<Vec3>>);

impl Image {
    fn compute(nx: usize, ny: usize, f: impl Fn(usize, usize) -> Vec3 + Sync) -> Image {
        Image(
            (0..ny)
                .into_par_iter()
                .rev()
                .map(|y| (0..nx).map(|x| f(x, y)).collect())
                .collect(),
        )
    }
}

fn print_ppm(image: Image) {
    println!("P3\n{} {}\n255", image.0[0].len(), image.0.len());
    for scanline in image.0 {
        for col in scanline {
            let col = Vec3(col.0.sqrt(), col.1.sqrt(), col.2.sqrt());

            let ir = (255.99 * col[R]) as i32;
            let ig = (255.99 * col[G]) as i32;
            let ib = (255.99 * col[B]) as i32;

            println!("{} {} {}", ir, ig, ib);
        }
    }
}

fn main() {
    const NX: usize = 200;
    const NY: usize = 100;
    const NS: usize = 50;

    let world = random_scene();

    let look_from = Vec3(13., 2., 3.);
    let look_at = Vec3(0., 0., 0.);
    let dist_to_focus = 10.;
    let aperture = 0.1;

    let camera = Camera::look(
        look_from,
        look_at,
        Vec3(0., 1., 0.),
        20.,
        NX as f32 / NY as f32,
        aperture,
        dist_to_focus,
    );

    let image = Image::compute(NX, NY, |x, y| {
        let col: Vec3 = (0..NS)
            .map(|_| {
                let mut rng = rand::thread_rng();
                let u = (x as f32 + rng.gen::<f32>()) / NX as f32;
                let v = (y as f32 + rng.gen::<f32>()) / NY as f32;
                let r = camera.get_ray(u, v);
                color(&world, r)
            })
            .sum();
        col / NS as f32
    });
    print_ppm(image);
}
