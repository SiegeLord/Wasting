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
pub struct Connection
{
	pub child: Option<hecs::Entity>,
}

#[derive(Debug, Copy, Clone)]
pub struct Ship;

#[derive(Debug, Copy, Clone)]
pub struct Car
{
	pub attached: bool,
}

#[derive(Debug, Copy, Clone)]
pub struct AffectedByGravity;

#[derive(Copy, Clone, Debug)]
pub enum CollideKind
{
	Ship,
	Car,
}

impl CollideKind
{
	pub fn collides_with(&self, other: &CollideKind) -> bool
	{
		match (self, other)
		{
			(CollideKind::Ship, CollideKind::Ship) => true,
			(CollideKind::Ship, CollideKind::Car) => true,
			(CollideKind::Car, CollideKind::Ship) => true,
			(CollideKind::Car, CollideKind::Car) => false,
		}
	}
}

#[derive(Copy, Clone, Debug)]
pub struct Solid
{
	pub size: f32,
	pub kind: CollideKind,
}

#[derive(Copy, Clone, Debug)]
pub struct CarCorpse
{
	pub multiplier: f32,
	pub time_to_die: f64,
}

#[derive(Copy, Clone, Debug)]
pub struct TimeToDie
{
	pub time_to_die: f64,
}

#[derive(Copy, Clone, Debug)]
pub enum DrawKind
{
	Ship,
	Car,
}

#[derive(Copy, Clone, Debug)]
pub struct Drawable
{
	pub kind: DrawKind,
}

#[derive(Clone, Debug)]
pub struct Sprite
{
	pub sprite: String,
}

#[derive(Clone, Debug)]
pub struct Engine
{
	pub on: bool,
	pub sprite: String,
}
