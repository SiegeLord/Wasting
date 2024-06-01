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

pub fn spawn_obj(pos: Point2<f32>, world: &mut hecs::World) -> Result<hecs::Entity>
{
	let entity = world.spawn((comps::Position { pos: pos },));
	Ok(entity)
}

struct MapCell
{
	ground: Vec<(f32, f32)>,
}

impl MapCell
{
	fn new(state: &mut game_state::GameState) -> Self
	{
		let num_points = 96;
		let mut ground = Vec::with_capacity(num_points);
		//let mut dx = 0.;
		//let mut dy = state.buffer_height() - 100.;
		//let mut y = dy;
		//let mut di = 0;
		//let mut rng = StdRng::seed_from_u64(0);
		let mut rng = thread_rng();
		//let mut len = rng.gen_range(3..10);
		//let w = state.buffer_width() / (num_points - 1) as f32;

		//for i in 0..num_points
		//{
		//	let rx = i as f32 * w;
		//	if i == di + len
		//	{
		//		di = i;
		//		dx = rx;
		//		dy = y;
		//		len = rng.gen_range(3..10);
		//        dbg!(len);
		//	}

		//	let x = w + rx - dx;
		//	let a = -0.02;
		//	let b = 1.35;
		//	let c = dy;
		//	y = a * x * x + b * x + c;
		//	ground.push((rx, y));
		//}

		let w = state.buffer_width() / (num_points - 1) as f32;

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

		Self { ground: ground }
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
}

impl Map
{
	fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		let mut world = hecs::World::new();
		spawn_obj(Point2::new(100., 100.), &mut world)?;

		Ok(Self {
			world: world,
			cell: MapCell::new(state),
		})
	}

	fn logic(&mut self, state: &mut game_state::GameState)
		-> Result<Option<game_state::NextScreen>>
	{
		let mut to_die = vec![];

		// Input.
		for (_, position) in self.world.query::<&mut comps::Position>().iter()
		{
			if state.controls.get_action_state(controls::Action::Move) > 0.5
			{
				position.pos.x += 10.;
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

		// Blob
		for (_, position) in self.world.query::<&comps::Position>().iter()
		{
			state.prim.draw_filled_circle(
				position.pos.x,
				position.pos.y,
				16.,
				Color::from_rgb_f(1.0, 0.0, 1.0),
			);
		}

		self.cell.draw(state);

		Ok(())
	}
}
