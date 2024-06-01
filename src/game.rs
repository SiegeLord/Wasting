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

const MAX_VEL: f32 = 20.;

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
		comps::Ship,
		comps::AffectedByGravity,
		comps::Solid {
			kind: comps::CollideKind::Ship,
			size: 16.,
		},
		comps::Drawable {
			kind: comps::DrawKind::Ship,
		},
		comps::Connection { child: None },
	));
	Ok(entity)
}

pub fn spawn_car(pos: Point2<f32>, world: &mut hecs::World) -> Result<hecs::Entity>
{
	let entity = world.spawn((
		comps::Position { pos: pos, dir: 0. },
		comps::Velocity {
			pos: Vector2::new(0., 0.),
			dir: 0.,
		},
		comps::Car { attached: false },
		comps::Solid {
			kind: comps::CollideKind::Car,
			size: 8.,
		},
		comps::Drawable {
			kind: comps::DrawKind::Car,
		},
		comps::Connection { child: None },
	));
	Ok(entity)
}

pub fn spawn_car_corpse(
	pos: Point2<f32>, vel: Vector2<f32>, time_to_die: f64, multiplier: f32, world: &mut hecs::World,
) -> Result<hecs::Entity>
{
	let entity = world.spawn((
		comps::Position { pos: pos, dir: 0. },
		comps::Velocity { pos: vel, dir: 0. },
		comps::Drawable {
			kind: comps::DrawKind::Car,
		},
		comps::CarCorpse {
			multiplier: multiplier,
			time_to_die: time_to_die,
		},
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
	fn new(rng: &mut impl Rng, state: &mut game_state::GameState) -> Self
	{
		let num_points = 96;
		let mut ground = Vec::with_capacity(num_points);
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

	fn collide(&self, pos: Point2<f32>, size: f32) -> Option<(f32, Point2<f32>)>
	{
		let num_points = self.ground.len();
		let w = self.width / (num_points - 1) as f32;

		for i in 1..num_points
		{
			let x1 = (i - 1) as f32 * w;
			let y1 = self.ground[i - 1].1;
			let x2 = i as f32 * w;
			let y2 = self.ground[i].1;
			let nearest = utils::nearest_line_point(Point2::new(x1, y1), Point2::new(x2, y2), pos);
			if (nearest - pos).norm() < size
			{
				return Some((((y2 - y1) / (x2 - x1)).abs(), nearest));
			}
		}
		None
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
	rng: StdRng,
	score: i32,
	target_score: i32,
	last_score_change: i32,
	score_message: String,
	last_score_time: f64,
}

impl Map
{
	fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		let mut world = hecs::World::new();
		let player = spawn_ship(Point2::new(100., 100.), -utils::PI / 2., &mut world)?;

		for i in 0..5
		{
			spawn_car(Point2::new(200. + i as f32 * 32., 100.), &mut world)?;
		}

		let mut rng = StdRng::seed_from_u64(0);

		Ok(Self {
			world: world,
			cell: MapCell::new(&mut rng, state),
			player: player,
			rng: rng,
			score: 0,
			target_score: 0,
			last_score_change: 0,
			score_message: "".to_string(),
			last_score_time: 0.,
		})
	}

	fn logic(&mut self, state: &mut game_state::GameState)
		-> Result<Option<game_state::NextScreen>>
	{
		let mut to_die = vec![];

		// Player respawn.
		if !self.world.contains(self.player)
		{
			self.player = spawn_ship(
				Point2::new(state.buffer_width() / 2., 64.),
				-utils::PI / 2.,
				&mut self.world,
			)?;
			self.score_message = format!("-{}", 1000.);
			self.last_score_change = -1000;
			self.target_score += self.last_score_change;
			self.last_score_time = state.time();
		}

		// Score.
		let delta = (utils::DT * (self.target_score - self.score) as f32) as i32;
		self.score += delta;
		if delta == 0 && self.score != self.target_score
		{
			self.score = self.target_score;
		}

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
			velocity.pos += v * utils::DT * 96. * thrust;
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

		// Connection cleanup.
		for (_, connection) in self.world.query::<&mut comps::Connection>().iter()
		{
			if let Some(child) = connection.child
			{
				if !self.world.contains(child)
				{
					connection.child = None;
				}
			}
		}

		// Train logic.
		let mut children_to_move = vec![];
		for (_, (position, connection)) in self
			.world
			.query::<(&comps::Position, &comps::Connection)>()
			.iter()
		{
			if let Some(child) = connection.child
			{
				children_to_move.push((position.pos, child));
			}
		}

		for (pos, child) in children_to_move
		{
			let child_position = self
				.world
				.query_one_mut::<&mut comps::Position>(child)
				.unwrap();
			let dv = child_position.pos - pos;
			let new_dv = 24. * dv / dv.norm();
			child_position.pos = pos + new_dv;
		}

		// Object-object collision
		let mut collide_pairs = vec![];
		for (e1, (position1, solid1)) in self
			.world
			.query::<(&comps::Position, &comps::Solid)>()
			.iter()
		{
			for (e2, (position2, solid2)) in self
				.world
				.query::<(&comps::Position, &comps::Solid)>()
				.iter()
			{
				if e1 == e2 || !solid1.kind.collides_with(&solid2.kind)
				{
					continue;
				}
				let d = (position1.pos - position2.pos).norm();
				if d < (solid1.size + solid2.size)
				{
					collide_pairs.push((e1, *position1, *solid1, e2, *position2, *solid2));
				}
			}
		}
		for (e1, _position1, _solid1, e2, _position2, _solid2) in collide_pairs
		{
			if self.world.get::<&comps::Ship>(e1).is_ok()
				&& Ok(false) == self.world.get::<&comps::Car>(e2).map(|c| c.attached)
			{
				let ship = e1;
				let car = e2;

				let mut tail = ship;
				loop
				{
					let mut connection = self.world.get::<&mut comps::Connection>(tail)?;
					if let Some(new_tail) = connection.child
					{
						tail = new_tail;
					}
					else
					{
						connection.child = Some(car);
						break;
					}
				}
				let mut car = self.world.get::<&mut comps::Car>(car)?;
				car.attached = true;
			}
		}

		// Ground collision.
		let mut multiplier = 1.;
		let mut delete_tail = vec![];
		for (e, (position, velocity, solid)) in self
			.world
			.query::<(&mut comps::Position, &mut comps::Velocity, &comps::Solid)>()
			.iter()
		{
			if let Some((slope, ground_point)) = self.cell.collide(position.pos, solid.size)
			{
				let dv = position.pos - ground_point;
				position.pos = ground_point + dv * solid.size / dv.norm();
				position.dir = -utils::PI / 2.;

				let is_ship = self.world.get::<&comps::Ship>(e).is_ok();
				if is_ship
				{
					let m = (MAX_VEL - (velocity.pos.y + velocity.pos.x.abs())) / 5.;
					multiplier = utils::max(1., 0.5 * (m / 0.5).round());
				}

				let explode = if self.world.get::<&comps::Car>(e).is_ok()
					|| (is_ship && (velocity.pos.y > MAX_VEL || velocity.pos.x.abs() > MAX_VEL))
					|| slope > 1.
				{
					true
				}
				else
				{
					false
				};
				velocity.pos.x = 0.;
				velocity.pos.y = 0.;

				delete_tail.push((e, explode));
			}
		}

		let mut car_corpses = vec![];
		for (e, explode) in delete_tail
		{
			let mut count = 0usize;
			let mut tail = e;
			loop
			{
				if let Some((connection, position)) = self
					.world
					.query_one::<(&mut comps::Connection, &comps::Position)>(tail)?
					.get()
				{
					// Hack.
					if explode || tail != self.player
					{
						to_die.push(tail);
					}

					if self.world.get::<&comps::Car>(tail).is_ok()
					{
						car_corpses.push((
							position.pos,
							state.time() + count as f64 * 0.25,
							explode,
						));
					}

					if let Some(child) = connection.child
					{
						tail = child;
					}
					else
					{
						break;
					}
				}
				else
				{
					break;
				}
				count += 1;
			}
		}

		for (pos, time_to_die, explode) in car_corpses
		{
			let r = if explode { 1. } else { 0. };
			spawn_car_corpse(
				pos,
				Vector2::new(
					self.rng.gen_range(-32.0..32.0),
					self.rng.gen_range(-32.0..32.0),
				) * r,
				time_to_die,
				multiplier * (1. - r),
				&mut self.world,
			)?;

			if !explode
			{
				multiplier += 0.5;
			}
		}

		// Car corpse
		for (id, car_corpse) in self.world.query_mut::<&comps::CarCorpse>()
		{
			if state.time() > car_corpse.time_to_die
			{
				if car_corpse.multiplier != 0.
				{
					self.score_message = format!("+{}x{}", 100., car_corpse.multiplier);
					self.last_score_change = (car_corpse.multiplier as f32 * 100.) as i32;
					self.target_score += self.last_score_change;
					self.last_score_time = state.time();
				}
				to_die.push(id);
			}
		}

		// Time to die
		for (id, time_to_die) in self.world.query_mut::<&comps::TimeToDie>()
		{
			if state.time() > time_to_die.time_to_die
			{
				to_die.push(id);
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
		self.cell.draw(state);

		for (_, (position, drawable)) in self
			.world
			.query::<(&comps::Position, &comps::Drawable)>()
			.iter()
		{
			match drawable.kind
			{
				comps::DrawKind::Ship =>
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
				comps::DrawKind::Car =>
				{
					state.prim.draw_filled_circle(
						position.pos.x,
						position.pos.y,
						8.,
						Color::from_rgb_f(1.0, 1.0, 1.0),
					);
				}
			}
		}
		if let Ok(velocity) = self.world.query_one_mut::<&comps::Velocity>(self.player)
		{
			let (color, alert) = if velocity.pos.x.abs() > MAX_VEL
			{
				(Color::from_rgb_f(0.9, 0.1, 0.1), "!")
			}
			else
			{
				(Color::from_rgb_f(0.9, 0.9, 0.9), "")
			};
			state.core.draw_text(
				state.ui_font(),
				color,
				(state.buffer_width() / 2. - 200.).round(),
				(state.buffer_height() - 32.).round(),
				FontAlign::Left,
				&format!("vx: {:.1} m/s{}", velocity.pos.x, alert),
			);
			let (color, alert) = if velocity.pos.y > MAX_VEL
			{
				(Color::from_rgb_f(0.9, 0.1, 0.1), "!")
			}
			else
			{
				(Color::from_rgb_f(0.9, 0.9, 0.9), "")
			};
			state.core.draw_text(
				state.ui_font(),
				color,
				(state.buffer_width() / 2. + 50.).round(),
				(state.buffer_height() - 32.).round(),
				FontAlign::Left,
				&format!("vy: {:.1} m/s{}", velocity.pos.y, alert),
			);
		}
		state.core.draw_text(
			state.ui_font(),
			Color::from_rgb_f(0.9, 0.9, 0.1),
			(state.buffer_width() / 2. - 100.).round(),
			32.,
			FontAlign::Left,
			"SCORE:",
		);
		state.core.draw_text(
			state.ui_font(),
			Color::from_rgb_f(0.1, 0.9, 0.1),
			(state.buffer_width() / 2. + 16.).round(),
			32.,
			FontAlign::Left,
			&format!("{}", self.score),
		);

		let f = 1. - utils::clamp((state.time() - self.last_score_time) / 2., 0., 1.) as f32;
		let color = if self.last_score_change > 0
		{
			Color::from_rgba_f(f * 0.9, f * 0.9, f * 0.1, f)
		}
		else
		{
			Color::from_rgba_f(f * 0.9, f * 0.1, f * 0.1, f)
		};
		state.core.draw_text(
			state.ui_font(),
			color,
			(state.buffer_width() / 2. + 16.).round(),
			48.,
			FontAlign::Left,
			&self.score_message,
		);

		Ok(())
	}
}
