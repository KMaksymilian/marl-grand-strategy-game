//! Grid coordinates. Point uses nonnegative indices; Position retains signed placement coordinates.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Point(pub usize, pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}
