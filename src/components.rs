use crate::sprite;
use allegro::*;
use na::{Point2, Vector2};
use nalgebra as na;
use rand::prelude::*;

#[derive(Debug, Copy, Clone)]
pub struct Position
{
	pub pos: Point2<f32>,
	pub dir: f32,
}

#[derive(Debug, Copy, Clone)]
pub struct Velocity
{
	pub pos: Vector2<f32>,
	pub dir: f32,
}

#[derive(Debug, Copy, Clone)]
pub struct Player;

#[derive(Debug, Copy, Clone)]
pub struct AffectedByGravity;
