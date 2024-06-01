use crate::error::Result;
use crate::{astar, components as comps, controls, game_state, sprite, ui, utils};
use allegro::*;
use allegro_font::*;
use allegro_primitives::*;
use na::{
	Isometry3, Matrix4, Perspective3, Point2, Point3, Quaternion, RealField, Rotation2, Rotation3,
	Unit, Vector2, Vector3, Vector4,
};
use nalgebra as na;
use rand::prelude::*;

use std::collections::HashMap;

pub struct Game
{
	map: Map,
	subscreens: ui::SubScreens,
}

impl Game
{
	pub fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		Ok(Self {
			map: Map::new(state)?,
			subscreens: ui::SubScreens::new(),
		})
	}

	pub fn logic(
		&mut self, state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
	{
		if self.subscreens.is_empty()
		{
			self.map.logic(state)
		}
		else
		{
			Ok(None)
		}
	}

	pub fn input(
		&mut self, event: &Event, state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
	{
		state.controls.decode_event(event);
		match *event
		{
			Event::MouseAxes { x, y, .. } =>
			{
				if state.track_mouse
				{
					let (x, y) = state.transform_mouse(x as f32, y as f32);
					state.mouse_pos = Point2::new(x as i32, y as i32);
				}
			}
			_ => (),
		}
		if self.subscreens.is_empty()
		{
			let in_game_menu;
			match *event
			{
				Event::KeyDown {
					keycode: KeyCode::Escape,
					..
				} =>
				{
					in_game_menu = true;
				}
				_ =>
				{
					let res = self.map.input(event, state);
					if let Ok(Some(game_state::NextScreen::InGameMenu)) = res
					{
						in_game_menu = true;
					}
					else
					{
						return res;
					}
				}
			}
			if in_game_menu
			{
				self.subscreens
					.push(ui::SubScreen::InGameMenu(ui::InGameMenu::new(state)));
				state.paused = true;
			}
		}
		else
		{
			if let Some(action) = self.subscreens.input(state, event)
			{
				match action
				{
					ui::Action::MainMenu => return Ok(Some(game_state::NextScreen::Menu)),
					_ => (),
				}
			}
			if self.subscreens.is_empty()
			{
				state.paused = false;
			}
		}
		Ok(None)
	}

	pub fn draw(&mut self, state: &game_state::GameState) -> Result<()>
	{
		if !self.subscreens.is_empty()
		{
			state.core.clear_to_color(Color::from_rgb_f(0.0, 0.0, 0.0));
			self.subscreens.draw(state);
		}
		else
		{
			state.core.clear_to_color(Color::from_rgb_f(0.5, 0.5, 1.));
			self.map.draw(state)?;
		}
		Ok(())
	}

	pub fn resize(&mut self, state: &game_state::GameState)
	{
		self.subscreens.resize(state);
	}
}

pub fn spawn_ship(pos: Point2<f32>, dir: f32, world: &mut hecs::World) -> Result<hecs::Entity>
{
	let entity = world.spawn((
		comps::Position { pos: pos, dir: dir },
		comps::Velocity {
			pos: Vector2::new(0., 0.),
			dir: 0.,
		},
		comps::Player,
		comps::AffectedByGravity,
		comps::Collidable,
	));
	Ok(entity)
}

struct MapCell
{
	width: f32,
	ground: Vec<(f32, f32)>,
}

impl MapCell
{
	fn new(state: &mut game_state::GameState) -> Self
	{
		let num_points = 96;
		let mut ground = Vec::with_capacity(num_points);
		//let mut rng = StdRng::seed_from_u64(0);
		let mut rng = thread_rng();
		let width = state.buffer_width();

		let w = width / (num_points - 1) as f32;

		let num_segments = 12;

		let mut y1 = 0.;

		let landing_segment = rng.gen_range(1..num_segments - 1);
		for s in 0..num_segments
		{
			let segment = if s + 1 == num_segments
			{
				num_points - ground.len()
			}
			else
			{
				rng.gen_range(6..12)
			};
			let a = 600.;
			let b = -a;
			let c = 50.;
			let x = s as f32 / (num_segments - 1) as f32;

			let amp = a * x * x + b * x + c;

			let y2 = if s == landing_segment
			{
				y1
			}
			else
			{
				rng.gen_range(-1.0..=1.0) * amp
			};
			let a = -rng.gen_range(100.0..300.0);

			for i in 0..segment
			{
				let x = i as f32 / segment as f32;
				let c = y1;
				let b = y2 - a - c;
				let y = if s == landing_segment
				{
					y1
				}
				else
				{
					a * x * x + b * x + c
				};
				ground.push((ground.len() as f32 * w, 300. + y));
			}
			y1 = y2;
		}

		Self {
			ground: ground,
			width: width,
		}
	}

	fn get_height(&self, x: f32) -> f32
	{
		let num_points = self.ground.len();
		let w = self.width / (num_points - 1) as f32;
		let idx = (x / w) as usize;
		let y1 = self.ground[idx].1;
		let y2 = self.ground[idx + 1].1;
		let f = (x - w * idx as f32) / w;
		f * y1 + (1. - f) * y2
	}

	fn draw(&self, state: &game_state::GameState)
	{
		state.prim.draw_polyline(
			&self.ground,
			LineJoinType::Bevel,
			LineCapType::Round,
			Color::from_rgb_f(1., 1., 1.),
			2.,
			0.5,
		);
	}
}

struct Map
{
	world: hecs::World,
	cell: MapCell,
	player: hecs::Entity,
}

impl Map
{
	fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		let mut world = hecs::World::new();
		let player = spawn_ship(Point2::new(100., 100.), -utils::PI / 2., &mut world)?;

		Ok(Self {
			world: world,
			cell: MapCell::new(state),
			player: player,
		})
	}

	fn logic(&mut self, state: &mut game_state::GameState)
		-> Result<Option<game_state::NextScreen>>
	{
		let mut to_die = vec![];

		// Player input.
		//let want_left = state.controls.get_action_state(controls::Action::Left) > 0.5;
		//let want_right = state.controls.get_action_state(controls::Action::Right) > 0.5;
		//let want_thrust = state.controls.get_action_state(controls::Action::Thrust) > 0.5;

		if let Ok((position, velocity)) = self
			.world
			.query_one_mut::<(&mut comps::Position, &mut comps::Velocity)>(self.player)
		{
			let right_left = state.controls.get_action_state(controls::Action::Right)
				- state.controls.get_action_state(controls::Action::Left);
			position.dir += 1. * utils::DT * right_left;
			let rot = Rotation2::new(position.dir);
			let v = rot * Vector2::new(1., 0.);

			let thrust = state.controls.get_action_state(controls::Action::Thrust);
			velocity.pos += v * utils::DT * 64. * thrust;
		}

		// Gravity.
		for (_, (velocity, _)) in self
			.world
			.query_mut::<(&mut comps::Velocity, &comps::AffectedByGravity)>()
		{
			velocity.pos.y += 24. * utils::DT;
		}

		// Physics.
		for (_, (position, velocity)) in self
			.world
			.query_mut::<(&mut comps::Position, &mut comps::Velocity)>()
		{
			position.pos += velocity.pos * utils::DT;
			position.dir += velocity.dir * utils::DT;
		}

		// Collision.
		for (_, (position, velocity, _)) in self.world.query_mut::<(
			&mut comps::Position,
			&mut comps::Velocity,
			&comps::Collidable,
		)>()
		{
			// TODO: Better collision.
			let ground_y = self.cell.get_height(position.pos.x);
			if position.pos.y > ground_y
			{
				position.pos.y = ground_y;
				velocity.pos.x = 0.;
				velocity.pos.y = 0.;
			}
		}

		// Remove dead entities
		to_die.sort();
		to_die.dedup();
		for id in to_die
		{
			//println!("died {id:?}");
			self.world.despawn(id)?;
		}

		Ok(None)
	}

	fn input(
		&mut self, _event: &Event, _state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
	{
		Ok(None)
	}

	fn draw(&mut self, state: &game_state::GameState) -> Result<()>
	{
		state.core.clear_to_color(Color::from_rgb_f(0., 0.0, 0.5));

		for (_, position) in self.world.query::<&comps::Position>().iter()
		{
			state.prim.draw_filled_circle(
				position.pos.x,
				position.pos.y,
				16.,
				Color::from_rgb_f(1.0, 0.0, 1.0),
			);
			let rot = Rotation2::new(position.dir);
			let v = rot * Vector2::new(1., 0.) * 16.;

			state.prim.draw_filled_circle(
				position.pos.x + v.x,
				position.pos.y + v.y,
				8.,
				Color::from_rgb_f(1.0, 0.0, 1.0),
			);
		}

		self.cell.draw(state);

		Ok(())
	}
}
