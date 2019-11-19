#![allow(clippy::type_complexity, dead_code)]

mod pass;

pub use imgui;
pub use pass::DrawImguiDesc;

use amethyst::{
	assets::Handle,
	core::legion::*,
	error::Error,
	input::{BindingTypes, InputEvent},
	renderer::{
		legion::bundle::{RenderOrder, RenderPlan, RenderPlugin, Target},
		rendy::{factory::Factory, graph::render::RenderGroupDesc},
		types::Backend,
		Texture,
	},
	shrev::EventChannel,
	window::Window,
	winit::Event,
};
use derivative::Derivative;
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use std::sync::{Arc, Mutex};

pub struct ImguiState {
	pub context: imgui::Context,
	pub textures: Vec<Handle<Texture>>,
}
unsafe impl Send for ImguiState {}

pub struct FilteredInputEvent<T: BindingTypes>(pub InputEvent<T>);

fn build_imgui_input_system<T: BindingTypes>(world: &mut World, config_flags: imgui::ConfigFlags) -> Box<dyn Schedulable> {
	let mut context = imgui::Context::create();

	context.fonts().add_font(&[imgui::FontSource::DefaultFontData {
		config: Some(imgui::FontConfig {
			size_pixels: 13.,
			..imgui::FontConfig::default()
		}),
	}]);
	context.fonts().tex_id = imgui::TextureId::from(std::usize::MAX);

	context.io_mut().config_flags |= config_flags;

	let mut platform = WinitPlatform::init(&mut context);
	platform.attach_window(context.io_mut(), &world.resources.get_mut::<Window>().unwrap(), HiDpiMode::Default);

	world.resources.insert(Arc::new(Mutex::new(ImguiState {
		context,
		textures: Vec::default(),
	})));
	world.resources.insert(platform);
	world.resources.insert(EventChannel::<FilteredInputEvent<T>>::default());

	let mut input_reader = world.resources.get_mut::<EventChannel<InputEvent<T>>>().unwrap().register_reader();
	let mut winit_reader = world.resources.get_mut::<EventChannel<Event>>().unwrap().register_reader();

	SystemBuilder::<()>::new("ImguiInputSystem")
		.read_resource::<Arc<Mutex<ImguiState>>>()
		.read_resource::<EventChannel<InputEvent<T>>>()
		.read_resource::<EventChannel<Event>>()
		.write_resource::<EventChannel<FilteredInputEvent<T>>>()
		.build(move |_, _, (state_mutex, input_events, winit_events, filtered_events), _| {
			let state = &mut state_mutex.lock().unwrap();
			let context = &mut state.context;

			for _ in winit_events.read(&mut winit_reader) {
				//platform.handle_event(state.io_mut(), &window, &event);
			}
			for input in input_events.read(&mut input_reader) {
				match input {
					InputEvent::MouseMoved { .. } |
					InputEvent::MouseButtonPressed(_) |
					InputEvent::MouseButtonReleased(_) |
					InputEvent::MouseWheelMoved(_) => {
						if !context.io().want_capture_mouse {
							filtered_events.single_write(FilteredInputEvent(input.clone()));
						}
					},
					InputEvent::KeyPressed { .. } | InputEvent::KeyReleased { .. } => {
						if !context.io().want_capture_keyboard {
							filtered_events.single_write(FilteredInputEvent(input.clone()));
						}
					},
					_ => filtered_events.single_write(FilteredInputEvent(input.clone())),
				}
			}
		})
}

static mut CURRENT_UI: Option<imgui::Ui<'static>> = None;

pub fn with(f: impl FnOnce(&imgui::Ui)) {
	unsafe {
		if let Some(ui) = current_ui() {
			(f)(ui);
		}
	}
}

/// # Safety
/// This function will cause UB if imgui is not initialized when it is called.
pub unsafe fn current_ui<'a>() -> Option<&'a imgui::Ui<'a>> { CURRENT_UI.as_ref() }

/// A [RenderPlugin] for rendering Imgui elements.
#[derive(Derivative)]
#[derivative(Debug(bound = ""))]
pub struct RenderImgui<T: BindingTypes> {
	target: Target,
	config_flags: imgui::ConfigFlags,
	_marker: std::marker::PhantomData<T>,
}
impl<T: BindingTypes> Default for RenderImgui<T> {
	#[cfg(feature = "docking")]
	fn default() -> Self {
		Self {
			target: Default::default(),
			_marker: Default::default(),
			config_flags: imgui::ConfigFlags::empty(),
		}
	}

	#[cfg(not(feature = "docking"))]
	fn default() -> Self {
		Self {
			target: Default::default(),
			_marker: Default::default(),
			config_flags: imgui::ConfigFlags::empty(),
		}
	}
}

impl<T: BindingTypes> RenderImgui<T> {
	pub fn with_imgui_config(mut self, config_flags: imgui::ConfigFlags) -> Self {
		self.config_flags = config_flags;
		self
	}

	/// Select render target on which UI should be rendered.
	pub fn with_target(mut self, target: Target) -> Self {
		self.target = target;
		self
	}
}

impl<B: Backend, T: BindingTypes> RenderPlugin<B> for RenderImgui<T> {
	fn on_build<'a, 'b>(&mut self, _: &mut World, dispatcher: &mut DispatcherBuilder) -> Result<(), Error> {
		let config_flags = self.config_flags;
		dispatcher.add_system(Stage::Begin, move |world| build_imgui_input_system::<T>(world, config_flags));

		Ok(())
	}

	fn should_rebuild(&mut self, _: &World) -> bool { false }

	fn on_plan(&mut self, plan: &mut RenderPlan<B>, _factory: &mut Factory<B>, _: &World) -> Result<(), Error> {
		plan.extend_target(self.target, |ctx| {
			ctx.add(RenderOrder::Overlay, DrawImguiDesc::new().builder())?;
			Ok(())
		});
		Ok(())
	}
}
