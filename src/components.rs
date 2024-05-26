use crate::sprite;
use allegro::*;
use na::Point2;
use nalgebra as na;
use rand::prelude::*;

#[derive(Debug, Copy, Clone)]
pub struct Position
{
	pub pos: Point2<f32>,
}
