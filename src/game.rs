use crate::error::Result;
use crate::{astar, components as comps, controls, game_state, sprite, ui, utils};
use allegro::*;
use allegro_audio::*;
use allegro_font::*;
use allegro_primitives::*;
use na::{
	Isometry3, Matrix4, Perspective3, Point2, Point3, Quaternion, RealField, Rotation2, Rotation3,
	Unit, Vector2, Vector3, Vector4,
};
use nalgebra as na;
use rand::prelude::*;

use std::collections::HashMap;

const MAX_VEL: f32 = 25.;
const SECTOR_SIZE: usize = 7;

pub struct Game
{
	map: Map,
	show_map: bool,
	subscreens: ui::SubScreens,
}

impl Game
{
	pub fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		state.cache_bitmap("data/bkg1.png")?;
		Ok(Self {
			map: Map::new(state)?,
			show_map: false,
			subscreens: ui::SubScreens::new(),
		})
	}

	pub fn logic(
		&mut self, state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
	{
		if self.subscreens.is_empty() && !self.show_map
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
		self.show_map = state.controls.get_action_state(controls::Action::ShowMap) > 0.5;
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
			let bitmap = state.get_bitmap("data/bkg1.png").unwrap();
			state.core.draw_bitmap(bitmap, 0., 0., Flag::zero());

			self.subscreens.draw(state);
		}
		else
		{
			state.core.clear_to_color(Color::from_rgb_f(0.5, 0.5, 1.));
			if self.show_map
			{
				self.map.draw_map(state)?;
			}
			else
			{
				self.map.draw(state)?;
			}
		}
		Ok(())
	}

	pub fn resize(&mut self, state: &game_state::GameState)
	{
		self.subscreens.resize(state);
	}
}

pub fn spawn_ship(
	sprite: String, engine: String, pos: Point2<f32>, dir: f32, world: &mut hecs::World,
	state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	state.cache_sprite(&sprite)?;
	state.cache_sprite(&engine)?;
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
		comps::Sprite { sprite: sprite },
		comps::Engine {
			sprite: engine,
			on: false,
		},
		comps::Connection { child: None },
	));
	Ok(entity)
}

pub fn spawn_car(
	pos: Point2<f32>, rng: &mut impl Rng, world: &mut hecs::World,
	state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let sprite = format!("data/car{}.cfg", rng.gen_range(1..5));
	state.cache_sprite(&sprite)?;
	let entity = world.spawn((
		comps::Position {
			pos: pos,
			dir: rng.gen_range(0.0..2.0 * utils::PI),
		},
		comps::Velocity {
			pos: Vector2::new(0., 0.),
			dir: *[-1., 1.].choose(rng).unwrap(),
		},
		comps::Car { attached: false },
		comps::Solid {
			kind: comps::CollideKind::Car,
			size: 8.,
		},
		comps::Sprite { sprite: sprite },
		comps::Connection { child: None },
	));
	Ok(entity)
}

pub fn spawn_star(
	pos: Point2<f32>, seed: usize, world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let sprite = format!("data/star{}.cfg", 1 + seed % 5);
	state.cache_sprite(&sprite)?;
	let entity = world.spawn((
		comps::Position { pos: pos, dir: 0. },
		comps::Doodad { sprite: sprite },
	));
	Ok(entity)
}

pub fn spawn_deliver(
	pos: Point2<f32>, world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let sprite = "data/deliver.cfg".to_string();
	state.cache_sprite(&sprite)?;
	let entity = world.spawn((
		comps::Position { pos: pos, dir: 0. },
		comps::Doodad { sprite: sprite },
		comps::TimeToDie {
			time_to_die: state.time() + 0.5,
		},
	));
	Ok(entity)
}

pub fn spawn_explosion(
	pos: Point2<f32>, world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let sprite = "data/explosion.cfg".to_string();
	state.cache_sprite(&sprite)?;
	let entity = world.spawn((
		comps::Position { pos: pos, dir: 0. },
		comps::Doodad { sprite: sprite },
		comps::TimeToDie {
			time_to_die: state.time() + 0.5,
		},
	));
	Ok(entity)
}

pub fn spawn_building(
	position: comps::Position, seed: usize, world: &mut hecs::World,
	state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let sprite = format!("data/building{}.cfg", 1 + seed % 2);
	state.cache_sprite(&sprite)?;
	let entity = world.spawn((position, comps::Doodad { sprite: sprite }));
	Ok(entity)
}

pub fn spawn_car_corpse(
	position: comps::Position, sprite: comps::Sprite, explode: bool, time_to_die: f64,
	multiplier: f32, rng: &mut impl Rng, world: &mut hecs::World,
) -> Result<hecs::Entity>
{
	let speed_mult = if explode { 1. } else { 0. };
	let entity = world.spawn((
		position,
		sprite,
		comps::Velocity {
			pos: Vector2::new(rng.gen_range(-32.0..32.0), rng.gen_range(-32.0..32.0)) * speed_mult,
			dir: rng.gen_range(-2.0..2.0) * speed_mult,
		},
		comps::CarCorpse {
			multiplier: multiplier,
			time_to_die: time_to_die,
			explode: explode,
		},
	));
	Ok(entity)
}

#[derive(Copy, Clone, Debug)]
enum Gravity
{
	None,
	Down(f32),
	Center(f32),
}

struct MapCell
{
	name: String,
	ground: Vec<(f32, f32)>,
	gravity: Gravity,
	population: i32,
	center: Point2<f32>,
	stars: Vec<Point2<f32>>,
	buildings: Vec<comps::Position>,
}

impl MapCell
{
	fn new(names: &mut Vec<String>, rng: &mut impl Rng, state: &mut game_state::GameState) -> Self
	{
		let num_points = 96;
		let mut ground = Vec::with_capacity(num_points + 2);
		let mut buildings = Vec::with_capacity(9);
		let population;
		let name;

		let num_stars = rng.gen_range(10..20);
		let mut stars = Vec::with_capacity(num_stars);
		for _ in 0..num_stars
		{
			stars.push(Point2::new(
				rng.gen_range(0.0..state.buffer_width()),
				rng.gen_range(0.0..state.buffer_height()),
			));
		}

		let center = Point2::new(
			state.buffer_width() / 2. + rng.gen_range(-16.0..16.0),
			state.buffer_height() / 2. + rng.gen_range(-16.0..16.0),
		);

		let strength = rng.gen_range(16.0..32.0);
		let choices = [
			(Gravity::Center(strength), 4),
			(Gravity::Down(strength), 1),
			(Gravity::None, 10),
		];
		let (gravity, _) = choices.choose_weighted(rng, |g_w| g_w.1).unwrap();

		match gravity
		{
			Gravity::Down(_) =>
			{
				let width = state.buffer_width();

				let w = width / (num_points - 1) as f32;

				let mut y1 = 0.;
				let mut segment_lengths = vec![];
				let mut cur_points = 0;
				loop
				{
					let segment = rng.gen_range(6..12);
					segment_lengths.push(segment);
					if segment + cur_points > num_points
					{
						break;
					}
					cur_points += segment;
				}
				let num_segments = segment_lengths.len();
				segment_lengths[num_segments - 1] = num_points - cur_points;
				let landing_segment = rng.gen_range(1..num_segments - 1);

				for (s, &segment) in segment_lengths.iter().enumerate()
				{
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
				let choices = [0, 1, 2, 3, 4, 5];
				population = *choices
					.choose_weighted(rng, |&p| {
						if p == 0
						{
							3
						}
						else
						{
							1
						}
					})
					.unwrap();
				ground.push((state.buffer_width(), state.buffer_height()));
				ground.push((0., state.buffer_height()));
				name = format!("{} System", names.pop().unwrap_or("Maximus".to_string()));

				for i in 0..9
				{
					let idx = 5 + i * 9;
					buildings.push(comps::Position {
						pos: Point2::new(ground[idx].0, ground[idx].1),
						dir: -utils::PI / 2.,
					});
				}
			}
			Gravity::Center(_) =>
			{
				let mut r1 = 0.;

				let mut segment_lengths = vec![];
				let mut cur_points = 0;
				loop
				{
					let segment = rng.gen_range(10..20);
					segment_lengths.push(segment);
					if segment + cur_points > num_points
					{
						break;
					}
					cur_points += segment;
				}
				let num_segments = segment_lengths.len();
				segment_lengths[num_segments - 1] = num_points - cur_points;
				let landing_segment = rng.gen_range(0..num_segments - 1);
				for (s, &segment) in segment_lengths.iter().enumerate()
				{
					let a = 60.;
					let b = -a;
					let c = 0.;
					let x = s as f32 / (num_segments - 1) as f32;

					let amp = a * x * x + b * x + c;
					let r2 = if s == landing_segment
					{
						r1
					}
					else
					{
						rng.gen_range(-1.0..=1.0) * amp
					};
					let a = rng.gen_range(100.0..150.0);

					for i in 0..segment
					{
						let x = i as f32 / segment as f32;
						let c = r1;
						let b = r2 - a - c;
						let r = if s == landing_segment
						{
							100. + r1
						}
						else
						{
							100. + a * x * x + b * x + c
						};
						let theta = 2. * utils::PI * ground.len() as f32 / num_points as f32;
						ground.push((r * theta.cos() + center.x, r * theta.sin() + center.y));
					}
					r1 = r2;
				}
				let choices = [0, 1, 2, 3, 4, 5];
				population = *choices
					.choose_weighted(rng, |&p| {
						if p == 0
						{
							6
						}
						else
						{
							1
						}
					})
					.unwrap();
				name = format!("{} System", names.pop().unwrap_or("Maximus".to_string()));
				for i in 0..9
				{
					let idx = 5 + i * 9;
					let pos = ground[idx];
					buildings.push(comps::Position {
						pos: Point2::new(pos.0, pos.1),
						dir: (pos.1 - center.y).atan2(pos.0 - center.x),
					});
				}
			}
			Gravity::None =>
			{
				population = 0;
				name = "Empty Space".to_string();
			}
		};

		buildings.shuffle(rng);

		Self {
			name: name,
			population: population,
			center: center,
			ground: ground,
			gravity: *gravity,
			stars: stars,
			buildings: buildings,
		}
	}

	fn collide(&self, pos: Point2<f32>, size: f32) -> Option<(f32, Vector2<f32>, Point2<f32>)>
	{
		let num_points = self.ground.len();
		for i in 1..num_points
		{
			let x1 = self.ground[i - 1].0;
			let y1 = self.ground[i - 1].1;
			let x2 = self.ground[i].0;
			let y2 = self.ground[i].1;
			let nearest = utils::nearest_line_point(Point2::new(x1, y1), Point2::new(x2, y2), pos);
			if (nearest - pos).norm() < size
			{
				let normal = -Vector2::new(y1 - y2, x2 - x1).normalize();
				let gravity_normal = match self.gravity
				{
					Gravity::None => Vector2::new(0., 0.),
					Gravity::Down(_) => Vector2::new(0., -1.),
					Gravity::Center(_) => (pos - self.center).normalize(),
				};
				return Some((normal.dot(&gravity_normal), normal, nearest));
			}
		}
		None
	}

	fn draw(&self, state: &game_state::GameState)
	{
		if self.ground.is_empty()
		{
			return;
		}
		// HACK
		let mut reversed = self.ground.clone();
		reversed.reverse();
		state
			.prim
			.draw_filled_polygon(&reversed, Color::from_rgb_f(0., 0., 0.05));
		match self.gravity
		{
			Gravity::Down(_) =>
			{
				state.prim.draw_polyline(
					&reversed[2..],
					LineJoinType::Bevel,
					LineCapType::Round,
					Color::from_rgb_f(0., 0., 0.9),
					2.,
					0.5,
				);
			}
			_ =>
			{
				state.prim.draw_polygon(
					&reversed,
					LineJoinType::Bevel,
					Color::from_rgb_f(0., 0., 0.9),
					2.,
					0.5,
				);
			}
		}
	}

	fn spawn_objects(
		&self, _total_pop: i32, rng: &mut impl Rng, world: &mut hecs::World,
		state: &mut game_state::GameState,
	) -> Result<()>
	{
		for (i, p) in self.stars.iter().enumerate()
		{
			spawn_star(*p, i, world, state)?;
		}

		for (i, p) in self.buildings[..self.population as usize]
			.iter()
			.enumerate()
		{
			spawn_building(*p, i, world, state)?;
		}

		let choices = [(0, 20), (1, 20), (2, 10), (3, 10), (10, 3), (20, 1)];
		let num = choices.choose_weighted(rng, |n_w| n_w.1).unwrap().0;
		match self.gravity
		{
			Gravity::None =>
			{
				for _ in 0..num
				{
					spawn_car(
						self.center
							+ Vector2::new(
								rng.gen_range(-256.0..256.0),
								rng.gen_range(-256.0..256.0),
							),
						rng,
						world,
						state,
					)?;
				}
			}
			Gravity::Center(_) =>
			{
				for _ in 0..num
				{
					let theta = rng.gen_range(0.0..2.0 * utils::PI);
					let r = 256.;
					spawn_car(
						self.center + Vector2::new(r * theta.cos(), r * theta.sin()),
						rng,
						world,
						state,
					)?;
				}
			}
			Gravity::Down(_) =>
			{
				for _ in 0..num
				{
					spawn_car(
						Point2::new(rng.gen_range(-256.0..256.0), rng.gen_range(0.0..256.0)),
						rng,
						world,
						state,
					)?;
				}
			}
		}
		Ok(())
	}
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum State
{
	Game,
	Victory,
	Defeat,
}

struct Map
{
	name: String,
	world: hecs::World,
	cells: Vec<MapCell>,
	cell_pos: Point2<usize>,
	player: hecs::Entity,
	rng: StdRng,
	score: i32,
	target_score: i32,
	last_score_change: i32,
	score_message: String,
	score_time: f64,
	pop_message: String,
	pop_time: f64,
	message: String,
	message_time: f64,
	day: i32,
	research: i32,
	strength: i32,
	max_train: i32,
	num_crashes: i32,
	state: State,
	num_cars_lost: i32,
	num_cars_delivered: i32,
	start_planets: i32,
	start_pop: i32,
	engine_sound: SampleInstance,
}

fn cell_idx(cell_pos: Point2<usize>) -> usize
{
	cell_pos.y * SECTOR_SIZE + cell_pos.x
}

fn get_total_pop(cells: &[MapCell]) -> i32
{
	let mut ret = 0;
	for cell in &cells[..]
	{
		ret += cell.population;
	}
	ret
}

impl Map
{
	fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		let mut world = hecs::World::new();
		let player = spawn_ship(
			state.player_ship(),
			state.player_engine(),
			Point2::new(state.buffer_width() / 2., 50.),
			-utils::PI / 2.,
			&mut world,
			state,
		)?;

		let mut rng = StdRng::seed_from_u64(thread_rng().gen());

		let mut names: Vec<_> = [
			"Bootus", "Bootset", "Albus", "Akyor", "Choron", "Kratus", "Abeles", "Aralor", "Kenji",
			"Jeren", "Gehen", "Multis", "X8532", "X532", "Wrandor", "Les-Lase", "Wender",
			"Minimus", "Drator", "Huru", "Klam", "Meled", "Tuts", "Qudro", "Merder", "Joo", "Zood",
			"Caestus", "Der", "Eol", "Iolus",
		]
		.iter()
		.map(|s| s.to_string())
		.collect();
		names.shuffle(&mut rng);

		let mut cells = vec![];
		let mut planets = 0;
		for _ in 0..SECTOR_SIZE * SECTOR_SIZE
		{
			let cell = MapCell::new(&mut names, &mut rng, state);
			if cell.population > 0
			{
				planets += 1;
			}
			cells.push(cell);
		}

		let total_pop = get_total_pop(&cells);
		cells[0].spawn_objects(total_pop, &mut rng, &mut world, state)?;

		Ok(Self {
			name: format!("{} Sector", names.pop().unwrap_or("Bratus".to_string())),
			world: world,
			cells: cells,
			cell_pos: Point2::new(0, 0),
			player: player,
			rng: rng,
			score: 0,
			target_score: 0,
			last_score_change: 0,
			score_message: "".to_string(),
			score_time: 0.,
			pop_message: "".to_string(),
			pop_time: 0.,
			message: format!(
				"Press {} to thrust.",
				state
					.options
					.controls
					.get_action_string(controls::Action::Thrust)
			),
			message_time: state.time(),
			day: 0,
			research: 0,
			strength: 1,
			max_train: 0,
			num_cars_lost: 0,
			num_cars_delivered: 0,
			num_crashes: 0,
			state: State::Game,
			start_pop: total_pop,
			start_planets: planets,
			engine_sound: state.sfx.play_continuous_sound(
				&format!(
					"data/engine{}.ogg",
					[1, 1, 2, 2, 1][state.options.player_engine as usize]
				),
				0.,
			)?,
		})
	}

	fn cell(&self) -> &MapCell
	{
		&self.cells[cell_idx(self.cell_pos)]
	}

	fn logic(&mut self, state: &mut game_state::GameState)
		-> Result<Option<game_state::NextScreen>>
	{
		if self.state != State::Game
		{
			return Ok(None);
		}
		let mut to_die = vec![];

		// Player respawn.
		if !self.world.contains(self.player)
		{
			self.player = spawn_ship(
				state.player_ship(),
				state.player_engine(),
				Point2::new(state.buffer_width() / 2., 50.),
				-utils::PI / 2.,
				&mut self.world,
				state,
			)?;
			self.score_message = format!("-{}", 1000.);
			self.last_score_change = -1000;
			self.target_score += self.last_score_change;
			self.score_time = state.time();
			self.num_crashes += 1;
		}

		// Score.
		let delta = (utils::DT * (self.target_score - self.score) as f32) as i32;
		self.score += delta;
		if delta == 0 && self.score != self.target_score
		{
			self.score = self.target_score;
		}

		// Player input.
		let want_left = state.controls.get_action_state(controls::Action::Left) > 0.5;
		let want_right = state.controls.get_action_state(controls::Action::Right) > 0.5;
		let want_thrust = state.controls.get_action_state(controls::Action::Thrust) > 0.5;

		if let Ok((position, velocity, engine)) = self.world.query_one_mut::<(
			&mut comps::Position,
			&mut comps::Velocity,
			&mut comps::Engine,
		)>(self.player)
		{
			let right_left = want_right as i32 as f32 - want_left as i32 as f32;
			position.dir += 2. * utils::DT * right_left;
			let rot = Rotation2::new(position.dir);
			let v = rot * Vector2::new(1., 0.);

			let thrust = want_thrust as i32 as f32;
			velocity.pos += v * utils::DT * 96. * thrust;

			engine.on = want_thrust;
			self.engine_sound
				.set_gain(if want_thrust { 1. } else { 0. })
				.unwrap();
		}

		// Gravity.
		let gravity = self.cell().gravity;
		let center = self.cell().center;
		for (_, (position, velocity, _)) in self.world.query_mut::<(
			&comps::Position,
			&mut comps::Velocity,
			&comps::AffectedByGravity,
		)>()
		{
			match gravity
			{
				Gravity::None => (),
				Gravity::Down(v) =>
				{
					velocity.pos.y += v * utils::DT;
				}
				Gravity::Center(v) =>
				{
					let mut dv = center - position.pos;
					if dv == Vector2::new(0., 0.)
					{
						dv = Vector2::new(1., 0.);
					}
					velocity.pos += v * dv / dv.norm() * utils::DT;
				}
			}
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
			let mut dv = child_position.pos - pos;
			if dv == Vector2::new(0., 0.)
			{
				dv = Vector2::new(1., 0.);
			}

			let dv = dv.normalize();
			child_position.pos = pos + 24. * dv;
			child_position.dir = dv.y.atan2(dv.x);
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
				state.sfx.play_sound("data/pickup.ogg")?;
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
			if let Some((dot, normal, ground_point)) = self.cell().collide(position.pos, solid.size)
			{
				let mut dv = position.pos - ground_point;
				if dv == Vector2::new(0., 0.)
				{
					dv = Vector2::new(1., 0.);
				}
				position.pos = ground_point + dv * solid.size / dv.norm();
				position.dir = normal.y.atan2(normal.x);

				let is_ship = self.world.get::<&comps::Ship>(e).is_ok();
				if is_ship
				{
					let m = (MAX_VEL - velocity.pos.norm()) / 5.;
					multiplier = utils::max(1., 0.5 * (m / 0.5).round());
				}

				let explode = if self.world.get::<&comps::Car>(e).is_ok()
					|| (is_ship && velocity.pos.norm() > MAX_VEL)
					|| dot < 0.9
				{
					true
				}
				else
				{
					false
				};
				velocity.pos.x = 0.;
				velocity.pos.y = 0.;

				if explode || (is_ship && self.cell().population > 0)
				{
					delete_tail.push((e, explode));
				}
			}
		}

		let mut car_corpses = vec![];
		let mut train_size = -1i32;
		let mut explosions = vec![];
		for (e, explode) in delete_tail
		{
			let mut count = 0usize;
			let mut tail = e;
			loop
			{
				let mut q = self
					.world
					.query_one::<(&mut comps::Connection, &comps::Position)>(tail)?;
				if let Some((connection, position)) = q.get()
				{
					// Hack.
					if explode || tail != self.player
					{
						to_die.push(tail);
					}

					if explode && tail == self.player
					{
						explosions.push((true, 1.0, position.pos));
					}

					if let Some((_, sprite)) = self
						.world
						.query_one::<(&comps::Car, &comps::Sprite)>(tail)?
						.get()
					{
						count += 1;
						if explode
						{
							self.num_cars_lost += 1;
						}
						else
						{
							train_size += 1;
							self.num_cars_delivered += 1;
						}
						car_corpses.push((
							position.clone(),
							sprite.clone(),
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
			}
		}
		self.max_train = utils::max(self.max_train, train_size);

		let mut add_pop = 0;
		for (position, sprite, time_to_die, explode) in car_corpses
		{
			spawn_car_corpse(
				position,
				sprite,
				explode,
				time_to_die,
				multiplier,
				&mut self.rng,
				&mut self.world,
			)?;

			if !explode
			{
				multiplier += 0.5;
				add_pop += 1;
			}
		}

		if add_pop > 0
		{
			if self.cell().population > 0
			{
				let cell = &mut self.cells[cell_idx(self.cell_pos)];
				let old_pop = cell.population;
				cell.population += add_pop;
				cell.population = utils::min(9, cell.population);
				let diff = cell.population - old_pop;
				if diff != 0
				{
					self.pop_message = format!("+{}", diff);
					self.pop_time = state.time();
				}
			}
		}

		// Car corpse
		for (id, (position, car_corpse)) in self
			.world
			.query_mut::<(&comps::Position, &comps::CarCorpse)>()
		{
			if state.time() > car_corpse.time_to_die
			{
				explosions.push((car_corpse.explode, car_corpse.multiplier, position.pos));
				if !car_corpse.explode
				{
					self.score_message = format!("+{}x{}", 100., car_corpse.multiplier);
					self.last_score_change = (car_corpse.multiplier as f32 * 100.) as i32;
					self.target_score += self.last_score_change;
					self.score_time = state.time();
				}
				to_die.push(id);
			}
		}

		for (explode, multiplier, pos) in explosions
		{
			if explode
			{
				state.sfx.play_sound("data/explosion.ogg")?;
				spawn_explosion(pos, &mut self.world, state)?;
			}
			else
			{
				state
					.sfx
					.play_sound_with_pitch("data/deliver.ogg", 1. + (multiplier - 1.) / 2.)?;
				spawn_deliver(pos, &mut self.world, state)?;
			}
		}

		// Transition
		let mut dir_and_pos = None;
		if let Ok(position) = self.world.query_one_mut::<&comps::Position>(self.player)
		{
			if position.pos.x > state.buffer_width() + 10.
			{
				self.cell_pos.x =
					(self.cell_pos.x as i32 + 1).rem_euclid(SECTOR_SIZE as i32) as usize;
				dir_and_pos = Some((0, position.pos));
			}
			if position.pos.y < -10.
			{
				self.cell_pos.y =
					(self.cell_pos.y as i32 - 1).rem_euclid(SECTOR_SIZE as i32) as usize;
				dir_and_pos = Some((1, position.pos));
			}
			if position.pos.x < -10.
			{
				self.cell_pos.x =
					(self.cell_pos.x as i32 - 1).rem_euclid(SECTOR_SIZE as i32) as usize;
				dir_and_pos = Some((2, position.pos));
			}
			if position.pos.y > state.buffer_height() + 10.
			{
				self.cell_pos.y =
					(self.cell_pos.y as i32 + 1).rem_euclid(SECTOR_SIZE as i32) as usize;
				dir_and_pos = Some((3, position.pos));
			}
		}

		let gravity = self.cell().gravity;
		if let Some((dir, pos)) = dir_and_pos
		{
			let mut pop_indices = vec![];
			for (i, cell) in self.cells.iter().enumerate()
			{
				if cell.population > 0
				{
					pop_indices.push(i);
				}
			}
			let old_research = self.research;
			let old_day = self.day;
			self.research += pop_indices.len() as i32;
			self.day += 1;
			println!("d: {} r: {}", self.day, self.research);

			let mut special_day = false;
			if self.day == 1
			{
				self.message = format!(
					"Press {}/{} to rotate.",
					state
						.options
						.controls
						.get_action_string(controls::Action::Left),
					state
						.options
						.controls
						.get_action_string(controls::Action::Right)
				);
				self.message_time = state.time();
				special_day = true;
			}
			else if self.day == 2
			{
				self.message = "Deliver supplies to\npopulated planets.".to_string();
				self.message_time = state.time();
				special_day = true;
			}
			else if self.day == 3
			{
				self.message = format!(
					"Hold {} to see sector map.",
					state
						.options
						.controls
						.get_action_string(controls::Action::ShowMap),
				);
				self.message_time = state.time();
				special_day = true;
			}
			if self.research >= 250 && old_research < 250
			{
				self.message = "Researchers see hints\nof a possible cure.".to_string();
				self.message_time = state.time();
				special_day = true;
			}
			else if self.research >= 500 && old_research < 500
			{
				self.message = "Desperate measures enable\na prototype innoculation.".to_string();
				self.message_time = state.time();
				special_day = true;
			}
			else if self.research >= 500 && old_research < 500
			{
				self.message = "Disastrous early trials\nilluminate path to salvation.".to_string();
				self.message_time = state.time();
				special_day = true;
			}
			else if self.research >= 1000 && old_research < 1000
			{
				state.sfx.play_sound("data/victory.ogg")?;
				self.message = format!("A triumph of science!\nYou have saved {}!.", self.name);
				self.message_time = state.time();
				self.strength = 0;
				self.state = State::Victory;
				special_day = true;
			}

			if self.research < 1000
			{
				if self.day >= 150 && old_day < 150
				{
					self.message = "The pathogen mutates to\nunfathomable deadliness.".to_string();
					self.message_time = state.time();
					self.strength = 2;
					special_day = true;
				}
				else if self.day >= 200 && old_day < 200
				{
					self.message =
						"The disease evolves to an\napocalyptic level of strength!".to_string();
					self.message_time = state.time();
					self.strength = 3;
					special_day = true;
				}
			}

			if !special_day && self.rng.gen_bool(0.5) && self.strength > 0
			{
				if let Some(&idx) = pop_indices.choose(&mut self.rng)
				{
					self.cells[idx].population =
						utils::max(0, self.cells[idx].population - self.strength);

					let name = &self.cells[idx].name;
					if self.cells[idx].population == 0
					{
						let messages = [
							(format!("{name} has been\nwiped out."), 4),
							(format!("There is no more\nillness at the {name}."), 4),
							(format!("{name} no longer\nrequires supplies."), 3),
							(format!("It is too late\nfor people of the {name}."), 3),
							(format!("{name} has gone silent."), 1),
						];
						self.message = messages
							.choose_weighted(&mut self.rng, |m_w| m_w.1)
							.unwrap()
							.0
							.clone();
					}
					else
					{
						let messages = [
							(format!("Hospitals are\noverwhelmed at the {name}."), 4),
							(format!("Illness takes for\nthe worse at the {name}."), 4),
							(format!("Disease spreads\nat the {name}."), 3),
							(format!("{name} is hit by\nthe infection."), 3),
							(format!("The living envy\nthe dead at the {name}."), 3),
							(format!("The end is near\nat the {name}."), 1),
						];
						self.message = messages
							.choose_weighted(&mut self.rng, |m_w| m_w.1)
							.unwrap()
							.0
							.clone();
					}
					self.message_time = state.time();
				}
			}
			if get_total_pop(&self.cells) == 0 && !pop_indices.is_empty()
			{
				state.sfx.play_sound("data/defeat.ogg")?;
				self.message = format!(
					"{} has no more people\nleft to save.\nYour services are no longer necessary.",
					self.name
				);
				self.message_time = state.time();
				self.state = State::Defeat;
			}

			let start_pos;
			let reset_vel;
			let delta;
			match gravity
			{
				Gravity::None | Gravity::Center(_) =>
				{
					reset_vel = false;
					match dir
					{
						0 =>
						{
							start_pos = Point2::new(0., pos.y);
							delta = Vector2::new(-10., 0.);
						}
						1 =>
						{
							start_pos = Point2::new(pos.x, state.buffer_height());
							delta = Vector2::new(0., 10.);
						}
						2 =>
						{
							start_pos = Point2::new(state.buffer_width(), pos.y);
							delta = Vector2::new(10., 0.);
						}
						3 =>
						{
							start_pos = Point2::new(pos.x, 0.);
							delta = Vector2::new(0., -10.);
						}
						_ => unreachable!(),
					}
				}
				Gravity::Down(_) =>
				{
					start_pos = Point2::new(state.buffer_width() / 2., 0.);
					delta = Vector2::new(0., -10.);
					reset_vel = true;
				}
			}
			let mut tail = self.player;
			let mut cur_pos = start_pos;
			loop
			{
				let (position, velocity, connection) = self
					.world
					.query_one_mut::<(
						&mut comps::Position,
						&mut comps::Velocity,
						&comps::Connection,
					)>(tail)
					.unwrap();
				position.pos = cur_pos;
				cur_pos += delta;
				if reset_vel
				{
					velocity.pos = Vector2::new(0., 0.);
					velocity.dir = 0.;
					position.dir = -utils::PI / 2.;
				}
				if let Some(new_tail) = connection.child
				{
					tail = new_tail;
				}
				else
				{
					break;
				}
			}

			for (e, car) in self.world.query_mut::<&comps::Car>()
			{
				if !car.attached
				{
					to_die.push(e);
				}
			}
			for (e, _) in self.world.query_mut::<&comps::Doodad>()
			{
				to_die.push(e);
			}
			let total_pop = get_total_pop(&self.cells);
			self.cells[cell_idx(self.cell_pos)].spawn_objects(
				total_pop,
				&mut self.rng,
				&mut self.world,
				state,
			)?;
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
		state.core.clear_to_color(Color::from_rgb_f(0., 0.0, 0.05));

		match self.state
		{
			State::Game =>
			{
				self.draw_game(state)?;
			}
			State::Victory =>
			{
				self.draw_victory(state)?;
			}
			State::Defeat =>
			{
				self.draw_defeat(state)?;
			}
		}

		Ok(())
	}

	fn draw_victory(&mut self, state: &game_state::GameState) -> Result<()>
	{
		let lh = state.ui_font().get_line_height() as f32;
		let center = Point2::new(state.buffer_width(), state.buffer_height()) / 2.;

		let mut y = center.y - 100.;

		let color = Color::from_rgb_f(0.9, 0.5, 0.5);

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			"Victory!",
		);
		y += lh;

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Score: {}", self.score),
		);
		y += lh;

		let mut num_planets = 0;
		let mut total_pop = 0;
		for cell in &self.cells
		{
			total_pop += cell.population;
			if cell.population > 0
			{
				num_planets += 1;
			}
		}

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Population: {}/{}", total_pop, self.start_pop),
		);
		y += lh;

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Planets: {}/{}", num_planets, self.start_planets),
		);
		y += lh;

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Days: {}", self.day),
		);
		y += lh;

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Crashes: {}", self.num_crashes),
		);
		y += lh;

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Longest train: {}", self.max_train),
		);
		y += lh;

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Supplies delivered: {}", self.num_cars_delivered),
		);
		y += lh;

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Supplies lost: {}", self.num_cars_lost),
		);
		//y += lh;

		Ok(())
	}
	fn draw_defeat(&mut self, state: &game_state::GameState) -> Result<()>
	{
		let lh = state.ui_font().get_line_height() as f32;
		let center = Point2::new(state.buffer_width(), state.buffer_height()) / 2.;

		let mut y = center.y - 100.;

		let color = Color::from_rgb_f(0.9, 0.5, 0.5);

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			"Defeat!",
		);
		y += lh;

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Score: {}", self.score),
		);
		y += lh;

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Cure: {}%", 100 * self.research / 1000),
		);
		y += lh;

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Days: {}", self.day),
		);
		y += lh;

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Crashes: {}", self.num_crashes),
		);
		y += lh;

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Longest train: {}", self.max_train),
		);
		y += lh;

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Supplies delivered: {}", self.num_cars_delivered),
		);
		y += lh;

		state.core.draw_text(
			state.ui_font(),
			color,
			center.x,
			y.round(),
			FontAlign::Centre,
			&format!("Supplies lost: {}", self.num_cars_lost),
		);
		//y += lh;
		Ok(())
	}

	fn draw_game(&mut self, state: &game_state::GameState) -> Result<()>
	{
		let lh = state.ui_font().get_line_height() as f32;
		let center = Point2::new(state.buffer_width(), state.buffer_height()) / 2.;

		state.core.hold_bitmap_drawing(true);
		for (_, (position, star)) in self
			.world
			.query::<(&comps::Position, &comps::Doodad)>()
			.iter()
		{
			let sprite = state.get_sprite(&star.sprite).unwrap();
			let variant = sprite.get_variant(state.time());
			// HACK: I drew the sprites wrong.
			sprite.draw_rotated(
				position.pos,
				variant,
				Color::from_rgb_f(1., 1., 1.),
				position.dir + utils::PI / 2.,
				state,
			);
		}
		state.core.hold_bitmap_drawing(false);

		self.cell().draw(state);

		state.core.hold_bitmap_drawing(true);
		for (_, (position, sprite)) in self
			.world
			.query::<(&comps::Position, &comps::Sprite)>()
			.iter()
		{
			let sprite = state.get_sprite(&sprite.sprite).unwrap();
			let variant = sprite.get_variant(state.time());
			// HACK: I drew the sprites wrong.
			sprite.draw_rotated(
				position.pos,
				variant,
				Color::from_rgb_f(1., 1., 1.),
				position.dir + utils::PI / 2.,
				state,
			);
		}

		for (_, (position, engine)) in self
			.world
			.query::<(&comps::Position, &comps::Engine)>()
			.iter()
		{
			if !engine.on
			{
				continue;
			}
			let sprite = state.get_sprite(&engine.sprite).unwrap();
			let variant = sprite.get_variant(state.time());
			// HACK: I drew the sprites wrong.
			sprite.draw_rotated(
				position.pos,
				variant,
				Color::from_rgb_f(1., 1., 1.),
				position.dir + utils::PI / 2.,
				state,
			);
		}
		state.core.hold_bitmap_drawing(false);

		if let Ok(velocity) = self.world.query_one_mut::<&comps::Velocity>(self.player)
		{
			let (color, alert) = if velocity.pos.norm() > MAX_VEL
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
				(state.buffer_width() / 2.).round(),
				(state.buffer_height() - lh - 32.).round(),
				FontAlign::Centre,
				&format!("Speed: {:.1} m/s{}", velocity.pos.norm(), alert),
			);
		}
		state.core.draw_text(
			state.ui_font(),
			Color::from_rgb_f(0.9, 0.9, 0.1),
			32.,
			32.,
			FontAlign::Left,
			"Score:",
		);
		state.core.draw_text(
			state.ui_font(),
			Color::from_rgb_f(0.1, 0.9, 0.1),
			(96. * state.options.ui_scale).round(),
			32.,
			FontAlign::Left,
			&format!("{}", self.score),
		);
		state.core.draw_text(
			state.ui_font(),
			Color::from_rgb_f(0.9, 0.9, 0.1),
			state.buffer_width() - 32.,
			32.,
			FontAlign::Right,
			&self.cell().name,
		);
		let gravity = match self.cell().gravity
		{
			Gravity::None => "None".to_string(),
			Gravity::Down(v) | Gravity::Center(v) => (v as i32).to_string(),
		};
		state.core.draw_text(
			state.ui_font(),
			Color::from_rgb_f(0.9, 0.9, 0.1),
			state.buffer_width() - 32.,
			32. + lh,
			FontAlign::Right,
			&format!("Gravity: {}", gravity),
		);
		if self.cell().population > 0
		{
			state.core.draw_text(
				state.ui_font(),
				Color::from_rgb_f(0.9, 0.9, 0.1),
				state.buffer_width() - 32.,
				32. + lh * 2.,
				FontAlign::Right,
				&format!("Pop: {}", self.cell().population),
			);
			let f = 1. - utils::clamp((state.time() - self.pop_time) / 2., 0., 1.) as f32;

			let color = Color::from_rgba_f(f * 0.9, f * 0.9, f * 0.1, f);
			state.core.draw_text(
				state.ui_font(),
				color,
				state.buffer_width() - 32.,
				32. + lh * 3.,
				FontAlign::Right,
				&self.pop_message,
			);
		}

		let f = 1. - utils::clamp((state.time() - self.score_time) / 2., 0., 1.) as f32;
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
			160.,
			32. + lh,
			FontAlign::Left,
			&self.score_message,
		);

		let f = 1. - utils::clamp((state.time() - self.message_time) / 6., 0., 1.) as f32;
		let color = Color::from_rgba_f(f * 0.1, f * 0.9, f * 0.5, f);
		for (i, message) in self.message.lines().enumerate()
		{
			state.core.draw_text(
				state.ui_font(),
				color,
				center.x.round(),
				(center.y + lh * i as f32).round(),
				FontAlign::Centre,
				&message,
			);
		}

		Ok(())
	}

	fn draw_map(&self, state: &game_state::GameState) -> Result<()>
	{
		state.core.clear_to_color(Color::from_rgb_f(0.0, 0.0, 0.0));
		let bitmap = state.get_bitmap("data/bkg1.png").unwrap();
		state.core.draw_bitmap(bitmap, 0., 0., Flag::zero());
		let center = Point2::new(state.buffer_width(), state.buffer_height()) / 2.;

		state.core.draw_text(
			state.ui_font(),
			Color::from_rgb_f(0.9, 0.9, 0.9),
			center.x.round(),
			32.,
			FontAlign::Centre,
			&self.name,
		);

		let total_pop = get_total_pop(&self.cells);
		let lh = state.ui_font().get_line_height() as f32;
		let pop_text = if total_pop > 0
		{
			format!("Population: {total_pop}")
		}
		else
		{
			"Population: Restless Dead".to_string()
		};
		state.core.draw_text(
			state.ui_font(),
			Color::from_rgb_f(0.9, 0.9, 0.9),
			center.x.round(),
			state.buffer_height() - 32. - lh,
			FontAlign::Centre,
			&pop_text,
		);

		for (i, cell) in self.cells.iter().enumerate()
		{
			let x = i % SECTOR_SIZE;
			let y = i / SECTOR_SIZE;
			let cell_w = 48.;
			let total_w = SECTOR_SIZE as f32 * cell_w;

			let fx = center.x - total_w / 2. + x as f32 * cell_w + cell_w / 2.;
			let fy = center.y - total_w / 2. + y as f32 * cell_w + cell_w / 2.;
			state.prim.draw_rectangle(
				fx - cell_w / 2.,
				fy - cell_w / 2.,
				fx + cell_w / 2.,
				fy + cell_w / 2.,
				Color::from_rgb_f(0.9, 0.1, 0.9),
				2.,
			);
			if Point2::new(x, y) == self.cell_pos
			{
				let f = 0.5 * ((10. * state.time()).cos() as f32 + 1.);
				state.prim.draw_rectangle(
					fx - cell_w / 2. + 2.,
					fy - cell_w / 2. + 2.,
					fx + cell_w / 2. - 2.,
					fy + cell_w / 2. - 2.,
					Color::from_rgb_f(0.9 * f, 0.9 * f, 0.9 * f),
					2.,
				);
			}

			match cell.gravity
			{
				Gravity::Down(_) =>
				{
					state
						.prim
						.draw_circle(fx, fy, 20., Color::from_rgb_f(0.0, 0.0, 0.9), 2.);
				}
				Gravity::Center(_) =>
				{
					state
						.prim
						.draw_circle(fx, fy, 13., Color::from_rgb_f(0.0, 0.0, 0.9), 2.);
				}
				_ => (),
			}

			if cell.population > 0
			{
				state.core.draw_text(
					state.ui_font(),
					Color::from_rgb_f(0.9, 0.9, 0.9),
					fx.round(),
					(fy - lh / 2.).round(),
					FontAlign::Centre,
					&format!("{}", cell.population),
				);
			}
		}

		Ok(())
	}
}
