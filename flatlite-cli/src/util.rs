use std::cmp::max;
use std::ops::{Add, Sub};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Vector2i {
    pub x: i32,
    pub y: i32,
}

impl Vector2i {
    pub fn new(x: i32, y: i32) -> Self {
        Vector2i { x, y }
    }

    pub fn col(&self) -> usize {
        self.x as usize
    }
    pub fn row(&self) -> usize {
        self.y as usize
    }

    pub fn clamp_wrapped(self, bounds: Vector2i) -> Self {
        let x = if self.x < 0 {
            max(bounds.x - 1, 0)
        } else if self.x >= bounds.x {
            0i32
        } else {
            self.x
        };
        let y = if self.y < 0 {
            max(bounds.y - 1, 0)
        } else if self.y >= bounds.y {
            0i32
        } else {
            self.y
        };

        Vector2i { x, y }
    }
}

impl Add<Vector2i> for Vector2i {
    type Output = Vector2i;

    fn add(self, rhs: Vector2i) -> Self::Output {
        Vector2i::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub<Vector2i> for Vector2i {
    type Output = Vector2i;

    fn sub(self, rhs: Vector2i) -> Self::Output {
        Vector2i::new(self.x - rhs.x, self.y - rhs.y)
    }
}
