// TODO: Find some way of disabling the `init_kernel` things. Perhaps `Plugin` binding for simplicity?

use kernel::KernelFunction;
use once_cell::sync::Lazy;
use std::env::current_exe;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use bevy::utils::synccell::SyncCell;
use luisa::prelude::*;
use luisa::runtime::KernelSignature;

pub use luisa_compute as luisa;

pub use {inventory, once_cell};

pub use bevy_luisa_macro::kernel;

pub mod prelude {
    pub use super::{
        execute_luisa_commands, execute_luisa_commands_delayed, synchronize_luisa_commands,
        Compute, LuisaCommandExt, LuisaCommands, LuisaCommandsType, LuisaDevice, LuisaPlugin,
    };
    pub use bevy_luisa_macro::kernel;
    pub use luisa::prelude::*;
    pub use luisa_compute as luisa;
}

extern crate self as bevy_luisa;

#[cfg(feature = "display")]
pub mod display;

mod kernel;

pub struct KernelCell<S: KernelSignature>(OnceLock<Kernel<S>>);

impl<S: KernelSignature> Deref for KernelCell<S> {
    type Target = Kernel<S>;
    fn deref(&self) -> &Self::Target {
        self.0.get().unwrap()
    }
}
impl<S: KernelSignature> KernelCell<S> {
    pub const fn default() -> Self {
        Self(OnceLock::new())
    }
    pub fn init(&self, kernel: Kernel<S>) {
        self.0.set(kernel).ok().unwrap();
    }
}

pub trait LuisaCommandsType: Send + Sync + 'static {
    fn debug_name() -> String {
        std::any::type_name::<Self>()
            .rsplit_once("::")
            .map(|x| x.1)
            .unwrap_or(std::any::type_name::<Self>())
            .to_string()
    }
}

pub struct Compute;
impl LuisaCommandsType for Compute {}

pub struct LuisaCommand {
    raw: SyncCell<Command<'static, 'static>>,
    debug_name: Option<String>,
}
impl From<Command<'static, 'static>> for LuisaCommand {
    fn from(raw: Command<'static, 'static>) -> Self {
        Self {
            raw: SyncCell::new(raw),
            debug_name: None,
        }
    }
}

pub trait LuisaCommandExt {
    fn debug(self, name: impl AsRef<str>) -> LuisaCommand;
}
impl LuisaCommandExt for Command<'static, 'static> {
    fn debug(self, name: impl AsRef<str>) -> LuisaCommand {
        LuisaCommand {
            raw: SyncCell::new(self),
            debug_name: Some(name.as_ref().to_string()),
        }
    }
}

#[derive(Resource)]
pub struct LuisaCommandsResource<T: LuisaCommandsType = Compute> {
    commands: Vec<LuisaCommand>,
    lock: Arc<Mutex<()>>,
    _marker: std::marker::PhantomData<T>,
}
impl<T: LuisaCommandsType> Default for LuisaCommandsResource<T> {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            lock: Arc::new(Mutex::new(())),
            _marker: std::marker::PhantomData,
        }
    }
}

#[derive(SystemParam)]
pub struct LuisaCommands<'w, T: LuisaCommandsType = Compute> {
    commands: ResMut<'w, LuisaCommandsResource<T>>,
}
impl<T: LuisaCommandsType> LuisaCommands<'_, T> {
    pub fn run(&mut self, command: impl Into<LuisaCommand>) -> &mut Self {
        self.commands.commands.push(command.into());
        self
    }
    pub fn run_all(
        &mut self,
        commands: impl IntoIterator<Item = impl Into<LuisaCommand>>,
    ) -> &mut Self {
        self.commands
            .commands
            .extend(commands.into_iter().map(Into::into));
        self
    }
}

#[derive(Resource, Deref)]
pub struct LuisaDevice(pub Device);
impl LuisaDevice {
    pub fn create_kernel_from_fn<S, F: KernelFunction<S>>(
        &self,
        options: &KernelBuildOptions,
        f: F,
    ) -> Kernel<F::Signature> {
        f.build(self, options.clone())
    }
    pub fn create_kernel_from_fn_with_name<S, F: KernelFunction<S>>(
        &self,
        options: &KernelBuildOptions,
        name: impl AsRef<str>,
        f: F,
    ) -> Kernel<F::Signature> {
        f.build(
            self,
            KernelBuildOptions {
                name: Some(name.as_ref().to_string()),
                ..options.clone()
            },
        )
    }
}

#[derive(Deref, DerefMut, Resource)]
pub struct DefaultKernelBuildOptions(pub KernelBuildOptions);

#[derive(Copy, Clone)]
#[allow(clippy::type_complexity)]
pub struct KernelRegistrationSystem(
    pub &'static Lazy<Mutex<Option<Box<dyn System<In = (), Out = ()>>>>>,
);

inventory::collect!(KernelRegistrationSystem);

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, SystemSet)]
pub enum KernelRegistrationSystemSet {
    Stage,
    ApplyCommands,
}

/// Synchronously executes all commands in the [`LuisaCommands<T>`] resource.
pub fn execute_luisa_commands<T: LuisaCommandsType>(
    device: LuisaDevice,
    mut commands: ResMut<LuisaCommandsResource<T>>,
) {
    let lock = commands.lock.clone();
    let _guard = lock.lock().unwrap();
    let commands = commands
        .commands
        .drain(..)
        .map(|c| {
            if let Some(name) = c.debug_name {
                trace!("[{}] {}", T::debug_name(), name);
            }
            SyncCell::to_inner(c.raw)
        })
        .collect::<Vec<_>>();
    let scope = device.0.default_stream().scope();
    scope.submit(commands);
    scope.synchronize();
    trace!("Synchronized [{}]", T::debug_name());
}

pub fn execute_luisa_commands_delayed<T: LuisaCommandsType>(
    device: LuisaDevice,
    mut commands: ResMut<LuisaCommandsResource<T>>,
) {
    let lock = commands.lock.clone();
    let commands = commands
        .commands
        .drain(..)
        .map(|c| {
            if let Some(name) = c.debug_name {
                trace!("[{}] {}", T::debug_name(), name);
            }
            SyncCell::to_inner(c.raw)
        })
        .collect::<Vec<_>>();
    let scope = device.0.default_stream().scope();
    AsyncComputeTaskPool::get()
        .spawn_local(async move {
            let _guard = lock.lock().unwrap();
            scope.submit(commands);
            scope.synchronize();
        })
        .detach();
}
pub fn synchronize_luisa_commands<T: LuisaCommandsType>(commands: Res<LuisaCommandsResource<T>>) {
    trace!("Synchronizing [{}]", T::debug_name());
    drop(commands.lock.lock().unwrap());
    trace!("Synchronized [{}]", T::debug_name());
}

pub struct LuisaPlugin<
    F: Fn(&mut App, Box<dyn System<In = (), Out = ()>>) + Send + Sync + 'static = fn(
        &mut App,
        Box<dyn System<In = (), Out = ()>>,
    ),
> {
    pub lib_path: PathBuf,
    pub device: String,
    pub default_kernel_build_options: KernelBuildOptions,
    pub kernel_build_system_callback: Option<F>,
}
impl Default for LuisaPlugin {
    fn default() -> Self {
        Self {
            lib_path: current_exe().unwrap(),
            device: "cuda".to_string(),
            default_kernel_build_options: KernelBuildOptions {
                async_compile: false,
                ..default()
            },
            kernel_build_system_callback: None,
        }
    }
}

impl<F: Fn(&mut App, Box<dyn System<In = (), Out = ()>>) + Send + Sync + 'static> Plugin
    for LuisaPlugin<F>
{
    fn build(&self, app: &mut App) {
        let lib_path = self.lib_path.clone();
        let device = self.device.clone();
        let ctx = Context::new(&lib_path);
        let device = ctx.create_device(&device);
        app.insert_resource(DefaultKernelBuildOptions(
            self.default_kernel_build_options.clone(),
        ))
        .insert_resource(LuisaDevice(device))
        .configure_sets(
            PostStartup,
            (
                KernelRegistrationSystemSet::Stage,
                KernelRegistrationSystemSet::ApplyCommands,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            apply_deferred.in_set(KernelRegistrationSystemSet::ApplyCommands),
        );
        for system in inventory::iter::<KernelRegistrationSystem> {
            let system = system.0.lock().unwrap().take().unwrap();
            if let Some(callback) = &self.kernel_build_system_callback {
                callback(app, system);
            } else {
                app.add_systems(
                    PostStartup,
                    system.in_set(KernelRegistrationSystemSet::Stage),
                );
            }
        }
    }
}
